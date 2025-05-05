use std::collections::HashMap;

use crate::domain::node_id;

#[derive(Debug, Clone)]
pub struct VolatileLeaderState {
    next_indices: HashMap<node_id::NodeId, usize>,
    match_indices: HashMap<node_id::NodeId, usize>,
}

impl VolatileLeaderState {
    /// When a leader first comes to power, it initializes all nextIndex values to the index just after
    /// the last one in its log.
    pub fn new(ids: impl Iterator<Item = node_id::NodeId>, last_log_index: usize) -> Self {
        let mut next_indices = HashMap::new();
        let mut match_indices = HashMap::new();

        for id in ids {
            next_indices.insert(id, last_log_index + 1);
            match_indices.insert(id, 0);
        }

        Self {
            next_indices,
            match_indices,
        }
    }

    pub fn get_next_index(&self, id: &node_id::NodeId) -> usize {
        *self.next_indices.get(id).unwrap()
    }

    pub fn set_next_index(&mut self, id: node_id::NodeId, index: usize) {
        self.next_indices.insert(id, index);
    }

    pub fn decrement_next_index(&mut self, id: node_id::NodeId) {
        self.next_indices.entry(id).and_modify(|index| {
            if *index > 1 {
                *index -= 1;
            }
        });
    }

    #[allow(dead_code)] // TODO: will use soon
    pub fn get_match_index(&self, id: &node_id::NodeId) -> usize {
        *self.match_indices.get(id).unwrap()
    }

    #[allow(dead_code)] // TODO: will use soon
    pub fn set_match_index(&mut self, id: node_id::NodeId, index: usize) {
        self.match_indices.insert(id, index);
    }

    #[allow(dead_code)] // TODO: will use soon
    pub fn decrement_match_index(&mut self, id: node_id::NodeId) {
        self.match_indices.entry(id).and_modify(|index| {
            if *index > 1 {
                *index -= 1;
            }
        });
    }

    /// The highest committable index is the integer just before the highest `next_index` value
    /// across all IDs.
    ///
    /// Using `&[1, 2, 2, 2, 3]` as an example:
    ///
    /// `&[1, 2, 2, 2, 3]` will be converted into a `HashMap` of IDs to each entry in the slice.
    /// Each entry represents the `next_index` for that ID, i.e. the index just after the last
    /// known replicated log index, used as the starting point for the next set of values the
    /// leader will send over to the node with that particular ID.
    ///
    /// The highest committable index is the value which can be accounted for across the majority
    /// of all others. To illustrate visually:
    ///
    /// ```no_run
    /// leader ->   (3)     followers ->                           (3)
    ///              |                                              |
    ///             (2)               ->         (2)   (2)   (2)   (2)
    ///              |                            |     |     |     |
    ///             (1)               ->   (1)   (1)   (1)   (1)   (1)
    ///              •                      •     •     •     •     •
    ///             ID0                    ID1   ID2   ID3   ID4   ID5
    /// ```
    ///
    /// Above you can see that whilst the leader and every follower covers index `1`, we can still
    /// get majority coverage at index 2.  Index 3 is only overed by the leadeer and one follower,
    /// so cannot count as the highest index covered by the majority.
    ///
    /// We then need to subtract 1 from the highest majority index, as the values in the map have
    /// been incremented by 1.  In this case, the majority can be described as being "ready to
    /// receive new entries starting from index 2", meaning they have confirmed all values up to
    /// index 1, and so we return 1 as our value.
    pub fn highest_committable_index(&self) -> Option<usize> {
        let mut indices = self.next_indices.values().collect::<Vec<_>>();

        indices.sort();

        match indices.len() {
            0 => None,
            len => Some(indices[len / 2].saturating_sub(1)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------
    #[test]
    fn initialises_as_expected() -> anyhow::Result<()> {
        for _ in 0..=20 {
            let mut ids = Vec::with_capacity(rand::random_range(1..=20));

            for _ in 0..ids.capacity() {
                ids.push(node_id::NodeId::new());
            }

            let last_log_index = rand::random_range(0..=100);
            let volatile_state = VolatileLeaderState::new(ids.clone().into_iter(), last_log_index);

            for id in ids.iter() {
                assert_eq!(volatile_state.get_next_index(id), last_log_index + 1);
                assert_eq!(volatile_state.get_match_index(id), 0);
            }
        }

        Ok(())
    }

    // -----------------------------------------------------
    fn run_highest_committable_index(
        next_indices: &[usize],
        expected: Option<usize>,
    ) -> anyhow::Result<()> {
        let max = next_indices.iter().max().unwrap_or(&0);
        assert!(next_indices.iter().all(|index| *index > 0 && index <= max));

        let mut ids = Vec::with_capacity(next_indices.len());
        for _ in 0..ids.capacity() {
            ids.push(node_id::NodeId::new());
        }

        let mut volatile_state = VolatileLeaderState::new(ids.clone().into_iter(), *max);
        next_indices.iter().enumerate().for_each(|(i, index)| {
            // Only way to get each nextIndex to the correct value, as there are no
            // accessible setters in the API.
            while volatile_state.get_next_index(&ids[i]) > *index {
                volatile_state.decrement_next_index(ids[i]);
            }
        });

        let actual = volatile_state.highest_committable_index();

        assert_eq!(
            expected, actual,
            "expected answer {expected:?} for {next_indices:?}, got {actual:?}. {:?}",
            volatile_state.next_indices
        );

        Ok(())
    }

    #[test]
    fn returns_the_highest_committable_index() -> anyhow::Result<()> {
        run_highest_committable_index(&[], None)?;
        run_highest_committable_index(&[1], Some(1 - 1))?;
        run_highest_committable_index(&[5, 4], Some(5 - 1))?;
        run_highest_committable_index(&[1, 2, 2, 2, 3], Some(2 - 1))?;
        run_highest_committable_index(&[2, 2, 3, 2, 5], Some(2 - 1))?;
        run_highest_committable_index(&[1, 2, 3, 4], Some(3 - 1))?;
        run_highest_committable_index(&[1, 2, 3, 4, 5], Some(3 - 1))?;
        run_highest_committable_index(&[1, 2, 4, 2, 5], Some(2 - 1))?;
        run_highest_committable_index(&[10, 10, 5, 5], Some(10 - 1))?;
        run_highest_committable_index(&[10, 5, 5], Some(5 - 1))?;

        Ok(())
    }
}
