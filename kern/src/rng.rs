use pi::rng::Rng as HwRng;

use crate::mutex::Mutex;

/// Global `Rng` singleton.
pub static RNG: Mutex<Rng> = Mutex::new(Rng::new());
/// A global singleton allowing read/write access to the console.
pub struct Rng {
    inner: Option<HwRng>,
}

impl Rng {
    /// Creates a new instance of `Rng`.
    const fn new() -> Rng {
        Rng { inner: None }
    }

    /// Initializes the Pi hardware Rng if it's not already initialized.
    #[inline]
    fn initialize(&mut self) {
        match self.inner {
            None => self.inner = Some(HwRng::new()),
            _ => (),
        }
    }

    /// Returns a mutable borrow to the inner `HwRng`, initializing it as
    /// needed.
    fn inner(&mut self) -> &mut HwRng {
        match self.inner {
            Some(ref mut rng) => rng,
            _ => {
                self.initialize();
                self.inner()
            }
        }
    }

    /// Gets a random number as a u32 that is greater than or equal to `min`
    /// and less than `max` blocking until there is sufficient entropy.
    pub fn rand(&mut self, min: u32, max: u32) -> u32 {
        self.inner().rand(min, max)
    }

    /// Gets a raw random number that is grater than or equal to 0 and
    /// less than or equal to `std::u32::MAX` blocking until there is
    /// sufficient entropy.
    pub fn r_rand(&mut self) -> u32 {
        self.inner().r_rand()
    }

    /// Gets the current entropy in the HwRng as a u32.
    pub fn entropy(&mut self) -> u32 {
        self.inner().entropy()
    }
}
