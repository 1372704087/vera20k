//! The binary heap gamemd's two path searches inline: the cell A*'s open list
//! (`PathfinderClass+0x14`, insert `0x0042A061..0x0042A122`, pop
//! `0x0042A2BB..0x0042A3B1`) and `Zone_precheck`'s (`0x0042C290`).
//!
//! Both compare strictly: an insert climbs only past a parent that costs
//! more, and a sift moves down only to a child that costs less, the left one
//! first, so equal costs keep their array order. A `BinaryHeap` with a tie-break
//! does not reproduce that order. The caller supplies the cost comparison.

/// A 0-based array heap; native index `i` (1-based) is index `i - 1` here.
#[derive(Debug)]
pub(super) struct NativeHeap<T>(Vec<T>);

impl<T> Default for NativeHeap<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T: Copy> NativeHeap<T> {
    pub(super) fn clear(&mut self) {
        self.0.clear();
    }

    /// The root, which the next pop returns.
    pub(super) fn first(&self) -> Option<&T> {
        self.0.first()
    }

    /// Insert `item`, climbing past each parent that `less` puts after it.
    pub(super) fn push(&mut self, item: T, less: impl Fn(&T, &T) -> bool) {
        let mut position = self.0.len();
        self.0.push(item);
        while position != 0 {
            let parent = (position - 1) / 2;
            if !less(&item, &self.0[parent]) {
                break;
            }
            self.0[position] = self.0[parent];
            position = parent;
        }
        self.0[position] = item;
    }

    /// Remove the root, moving the last item to the root and sifting it down.
    pub(super) fn pop(&mut self, less: impl Fn(&T, &T) -> bool) -> Option<T> {
        let first = *self.0.first()?;
        let last = self.0.pop().expect("nonempty heap");
        if !self.0.is_empty() {
            self.0[0] = last;
            self.sift_down(&less);
        }
        Some(first)
    }

    /// Put `item` at the root and sift it down, returning the old root. The
    /// A* takes its next node this way when the held child costs no less
    /// than the root (`0x0042A1E8`).
    pub(super) fn replace_first(&mut self, item: T, less: impl Fn(&T, &T) -> bool) -> T {
        let first = std::mem::replace(&mut self.0[0], item);
        self.sift_down(&less);
        first
    }

    fn sift_down(&mut self, less: &impl Fn(&T, &T) -> bool) {
        let mut position = 0;
        loop {
            let mut smallest = position;
            for child in [position * 2 + 1, position * 2 + 2] {
                if child < self.0.len() && less(&self.0[child], &self.0[smallest]) {
                    smallest = child;
                }
            }
            if smallest == position {
                break;
            }
            self.0.swap(position, smallest);
            position = smallest;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NativeHeap;

    /// (cost, arrival) pairs compared by cost alone.
    fn less(a: &(u32, u32), b: &(u32, u32)) -> bool {
        a.0 < b.0
    }

    #[test]
    fn equal_costs_keep_the_native_array_order() {
        let mut heap = NativeHeap::default();
        for (arrival, cost) in [5, 3, 3, 7, 3, 1, 3].into_iter().enumerate() {
            heap.push((cost, arrival as u32), less);
        }
        let mut order = Vec::new();
        while let Some(item) = heap.pop(less) {
            order.push(item);
        }
        // Not the arrival order among the threes, nor any tie-break: the
        // strict climbs and sifts leave them as the array has them.
        assert_eq!(
            order,
            [(1, 5), (3, 6), (3, 2), (3, 4), (3, 1), (5, 0), (7, 3)]
        );
    }

    #[test]
    fn replace_first_returns_the_root_and_sifts_the_new_item() {
        let mut heap = NativeHeap::default();
        for (arrival, cost) in [2, 4, 4, 6].into_iter().enumerate() {
            heap.push((cost, arrival as u32), less);
        }
        assert_eq!(heap.replace_first((4, 9), less), (2, 0));
        // The new root ties with its left child and stays put.
        assert_eq!(heap.first(), Some(&(4, 9)));
        assert_eq!(heap.replace_first((5, 8), less), (4, 9));
        assert_eq!(heap.first(), Some(&(4, 1)));
    }
}
