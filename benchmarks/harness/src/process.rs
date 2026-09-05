use std::{
    ffi::{OsStr, OsString},
    io::{self, BufRead, BufReader, Read},
    process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

const GRACE: Duration = Duration::from_secs(5);

pub struct ManagedChild {
    child: Child,
    stdout: Option<BufReader<ChildStdout>>,
    stdin: Option<ChildStdin>,
}

impl ManagedChild {
    pub fn spawn_self(args: &[OsString]) -> io::Result<Self> {
        let envs: [(OsString, OsString); 0] = [];
        Self::spawn_self_with_env(args, &envs)
    }
    pub fn spawn_self_with_env<K: AsRef<OsStr>, V: AsRef<OsStr>>(
        args: &[OsString],
        envs: &[(K, V)],
    ) -> io::Result<Self> {
        let child = Command::new(std::env::current_exe()?)
            .args(args)
            .envs(envs.iter().map(|(k, v)| (k.as_ref(), v.as_ref())))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        // Establish cleanup ownership before any fallible setup.
        let mut owned = Self {
            child,
            stdout: None,
            stdin: None,
        };
        crate::affinity::apply_parent_and_child_affinity_if_configured(&owned.child)?;
        owned.stdout = owned.child.stdout.take().map(BufReader::new);
        owned.stdin = owned.child.stdin.take();
        Ok(owned)
    }
    pub fn take_pipes(&mut self) -> io::Result<(ChildStdin, BufReader<ChildStdout>)> {
        Ok((
            self.stdin
                .take()
                .ok_or_else(|| io::Error::other("missing child stdin"))?,
            self.stdout
                .take()
                .ok_or_else(|| io::Error::other("missing child stdout"))?,
        ))
    }
    pub fn wait_for_ready(&mut self) -> io::Result<String> {
        let mut stdout = self
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("missing child stdout"))?;
        let (send, receive) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let mut line = String::new();
            let result = stdout
                .by_ref()
                .take(4096)
                .read_line(&mut line)
                .and_then(|count| {
                    if count == 0 || !line.ends_with('\n') {
                        Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "invalid child readiness",
                        ))
                    } else {
                        Ok(line.trim().to_owned())
                    }
                });
            let _ = send.send((stdout, result));
        });
        let (stdout, result) = receive.recv_timeout(GRACE).map_err(|_| {
            io::Error::new(io::ErrorKind::TimedOut, "child readiness deadline exceeded")
        })?;
        self.stdout = Some(stdout);
        result
    }
    pub fn request_shutdown(&mut self) {
        self.stdin.take();
    }
    pub fn wait(mut self) -> io::Result<ExitStatus> {
        self.request_shutdown();
        let deadline = Instant::now() + GRACE;
        loop {
            if let Some(status) = self.child.try_wait()? {
                crate::resources::record_child(&self.child);
                return Ok(status);
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "child shutdown deadline exceeded",
                ));
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}
impl Drop for ManagedChild {
    fn drop(&mut self) {
        self.stdin.take();
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
pub fn hold_until_stdin_closes() -> io::Result<()> {
    io::copy(&mut io::stdin(), &mut io::sink()).map(|_| ())
}

/// The parent joins a kill-on-close job before spawning, so descendants inherit ownership.
/// The watchdog has no polling overhead in the measured thread.
pub(crate) fn supervise(config: &crate::BenchmarkConfig) -> io::Result<()> {
    if config.role == crate::ProcessRole::Child {
        return Ok(());
    }
    #[cfg(windows)]
    let job = {
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            System::{
                JobObjects::{
                    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
                    SetInformationJobObject,
                },
                Threading::GetCurrentProcess,
            },
        };
        let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if job.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&info) as u32,
            )
        };
        if ok == 0 || unsafe { AssignProcessToJobObject(job, GetCurrentProcess()) } == 0 {
            let error = io::Error::last_os_error();
            unsafe {
                CloseHandle(job);
            }
            return Err(error);
        }
        // Held for process lifetime; OS closure covers abort and forced exit too.
        job as usize
    };
    let timeout = Duration::from_secs(config.timeout_seconds as u64);
    thread::spawn(move || {
        thread::sleep(timeout);
        eprintln!("process-tree supervisor deadline exceeded");
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(job as _, 124);
        }
        std::process::exit(124);
    });
    Ok(())
}
