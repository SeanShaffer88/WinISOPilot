//! Undo/redo history for the user's selection.

/// Bounded undo/redo stacks. `record` is called with the state *before* a
/// change; any new change clears the redo stack.
#[derive(Debug, Clone)]
pub struct History<T> {
    undo: Vec<T>,
    redo: Vec<T>,
    limit: usize,
}

impl<T: Clone + PartialEq> History<T> {
    pub fn new(limit: usize) -> Self {
        History { undo: Vec::new(), redo: Vec::new(), limit: limit.max(1) }
    }

    /// Remembers `before` unless it equals the latest entry.
    pub fn record(&mut self, before: &T) {
        if self.undo.last() != Some(before) {
            self.undo.push(before.clone());
            if self.undo.len() > self.limit {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
    }

    /// Returns the state to restore, saving `current` for redo.
    pub fn undo(&mut self, current: &T) -> Option<T> {
        let prev = self.undo.pop()?;
        self.redo.push(current.clone());
        Some(prev)
    }

    /// Returns the state to restore, saving `current` for undo.
    pub fn redo(&mut self, current: &T) -> Option<T> {
        let next = self.redo.pop()?;
        self.undo.push(current.clone());
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_cycle() {
        let mut h = History::new(3);
        let mut s = 0;
        for next in 1..=5 {
            h.record(&s);
            s = next;
        }
        // Only the last 3 states before changes are kept: 2, 3, 4.
        assert_eq!(h.undo(&s), Some(4));
        s = 4;
        assert_eq!(h.undo(&s), Some(3));
        s = 3;
        assert!(h.can_redo());
        assert_eq!(h.redo(&s), Some(4));
        s = 4;
        assert_eq!(h.undo(&s), Some(3));
        s = 3;
        assert_eq!(h.undo(&s), Some(2));
        s = 2;
        assert_eq!(h.undo(&s), None);
        // A new change clears redo.
        h.record(&s);
        assert!(!h.can_redo());
        let _ = s;
    }

    #[test]
    fn no_duplicate_entries() {
        let mut h = History::new(10);
        h.record(&1);
        h.record(&1);
        assert_eq!(h.undo(&2), Some(1));
        assert_eq!(h.undo(&1), None);
    }
}
