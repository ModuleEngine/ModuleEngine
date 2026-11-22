use std::{collections::VecDeque, ops::Range};

/// Offsets in the current staging buffer. Outstanding ranges are sorted by
/// graphics completion value, including ranges initially read on transfer.
#[derive(Default)]
pub(crate) struct StagingCursor {
    head: u64,
    used: VecDeque<(u64, Range<u64>)>,
}

impl StagingCursor {
    pub fn reserve(
        &mut self,
        size: u64,
        capacity: u64,
        alignment: u64,
        completed: u64,
    ) -> Option<Range<u64>> {
        while self
            .used
            .front()
            .is_some_and(|(value, _)| *value <= completed)
        {
            self.used.pop_front();
        }
        if size > capacity {
            return None;
        }
        let aligned = self.head.next_multiple_of(alignment);
        let start = if aligned + size <= capacity {
            aligned
        } else {
            0
        };
        let region = start..start + size;
        self.used
            .iter()
            .all(|(_, active)| region.start >= active.end || region.end <= active.start)
            .then_some(region)
    }

    pub fn commit(&mut self, serial: u64, region: Range<u64>) {
        self.head = region.end;
        self.used.push_back((serial, region));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staging_reuses_a_wrapped_range_only_after_that_submission_completes() {
        let mut ring = StagingCursor::default();
        let first = ring.reserve(13, 64, 16, 0).unwrap();
        assert_eq!(first, 0..13);
        ring.commit(3, first);
        let second = ring.reserve(35, 64, 16, 0).unwrap();
        assert_eq!(second, 16..51);
        ring.commit(4, second);
        assert_eq!(ring.reserve(13, 64, 16, 2), None);
        assert_eq!(ring.reserve(13, 64, 16, 3), Some(0..13));
        assert_eq!(ring.reserve(65, 64, 16, 4), None);
        // Dropping a recording leaves the range available for retry.
        assert_eq!(ring.reserve(13, 64, 16, 4), Some(0..13));
    }
}
