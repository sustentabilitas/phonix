/// Fixed-capacity circular buffer of the most recent audio samples.
///
/// Always-on: every sample pushed evicts the oldest once full. [`snapshot`] copies
/// the current contents oldest-first without disturbing the ring, so streaming can
/// continue immediately after a wake trigger.
pub struct PreRollRing {
    buf: Vec<f32>,
    head: usize, // index of the oldest sample when full; next write position
    len: usize,
}

impl PreRollRing {
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            buf: vec![0.0; capacity],
            head: 0,
            len: 0,
        }
    }

    pub fn push(&mut self, sample: f32) {
        let cap = self.buf.len();
        if self.len < cap {
            self.buf[self.len] = sample;
            self.len += 1;
        } else {
            self.buf[self.head] = sample;
            self.head = (self.head + 1) % cap;
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn is_full(&self) -> bool {
        self.len == self.buf.len()
    }

    /// Copy the current contents, oldest sample first.
    pub fn snapshot(&self) -> Vec<f32> {
        let cap = self.buf.len();
        let mut out = Vec::with_capacity(self.len);
        if self.len < cap {
            out.extend_from_slice(&self.buf[..self.len]);
        } else {
            out.extend_from_slice(&self.buf[self.head..]);
            out.extend_from_slice(&self.buf[..self.head]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_then_evicts_oldest_in_order() {
        let mut r = PreRollRing::new(3);
        r.push(1.0);
        r.push(2.0);
        assert_eq!(r.snapshot(), vec![1.0, 2.0]);
        assert!(!r.is_full());
        r.push(3.0);
        assert!(r.is_full());
        assert_eq!(r.snapshot(), vec![1.0, 2.0, 3.0]);
        r.push(4.0); // evicts 1.0
        assert_eq!(r.snapshot(), vec![2.0, 3.0, 4.0]);
        assert_eq!(r.len(), 3);
    }

    #[test]
    fn snapshot_is_non_destructive() {
        let mut r = PreRollRing::new(2);
        r.push(5.0);
        let a = r.snapshot();
        let b = r.snapshot();
        assert_eq!(a, b);
    }
}
