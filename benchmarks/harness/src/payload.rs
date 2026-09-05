use std::{cell::Cell, hint::black_box, io};

thread_local! {
    static FULL_VALIDATION: Cell<bool> = const { Cell::new(true) };
}

pub(crate) fn set_full_validation(full: bool) {
    FULL_VALIDATION.set(full);
}

/// The first eight wire bytes contain an unsigned little-endian request sequence.
/// The responder increments byte zero (without carry); the payload is otherwise echoed.
pub fn initialize_payload(buffer: &mut [u8]) {
    assert!(buffer.len() > 8);
    buffer[..8].fill(0);
    for (index, byte) in buffer[8..].iter_mut().enumerate() {
        *byte = (index % 251) as u8;
    }
}

pub fn check_response_and_advance(outbound: &mut [u8], inbound: &[u8]) -> io::Result<()> {
    if inbound.len() != outbound.len() || inbound.len() <= 8 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "incorrect response length",
        ));
    }
    let sequence = u64::from_le_bytes(outbound[..8].try_into().unwrap());
    let full = FULL_VALIDATION.get() || sequence.is_multiple_of(1024);
    let checked = if full { inbound.len() } else { 8 };
    for index in 0..checked {
        let expected = if index == 0 {
            outbound[0].wrapping_add(1)
        } else if index >= 8 {
            ((index - 8) % 251) as u8
        } else {
            outbound[index]
        };
        if inbound[index] != expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "corrupt/stale response: sequence={sequence}, byte={index}, expected={expected}, received={}",
                    inbound[index]
                ),
            ));
        }
    }
    black_box(inbound);
    // Preserve the fixed-copy workload. Expected payload bytes are computed independently
    // above, so copying a sampled unchecked byte cannot change the validation oracle.
    #[cfg(not(feature = "copy-elision"))]
    outbound.copy_from_slice(inbound);
    black_box(&mut *outbound);
    outbound[..8].copy_from_slice(&(sequence + 1).to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_corruption_short_and_stale_replies() {
        for size in [1, 2, 63, 64, 65, 4095, 4096, 4097] {
            let mut request = vec![0; size + 8];
            initialize_payload(&mut request);
            let mut response = request.clone();
            response[0] = response[0].wrapping_add(1);
            let mut corrupt = response.clone();
            corrupt[size + 7] ^= 1;
            assert!(check_response_and_advance(&mut request, &corrupt).is_err());
            assert!(check_response_and_advance(&mut request, &response[..size + 7]).is_err());
            check_response_and_advance(&mut request, &response).unwrap();
            assert!(check_response_and_advance(&mut request, &response).is_err());
        }
    }
}
