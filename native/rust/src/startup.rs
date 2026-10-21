//! A fresh native surface must present real UI before the host reveals it.
#[derive(Default)]
pub struct FirstFrame {
    presented: bool,
}

impl FirstFrame {
    pub fn invalidate(&mut self) {
        self.presented = false;
    }

    pub fn update(&mut self, painted: bool) -> bool {
        if !painted || self.presented {
            return false;
        }
        self.presented = true;
        true
    }
}

#[cfg(test)]
#[path = "../tests/unit/startup.rs"]
mod tests;
