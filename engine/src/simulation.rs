#[derive(Clone, Debug)]
pub struct Simulation {
    tick: u64,
    paused: bool,
}

impl Default for Simulation {
    fn default() -> Self {
        Self {
            tick: 0,
            paused: true,
        }
    }
}

impl Simulation {
    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn advance(&mut self, ticks: u32) -> u64 {
        if !self.paused {
            self.increment(ticks);
        }
        self.tick
    }

    pub fn step(&mut self) -> u64 {
        self.increment(1);
        self.tick
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    fn increment(&mut self, ticks: u32) {
        self.tick = self.tick.saturating_add(u64::from(ticks));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulation_starts_paused_at_tick_zero() {
        let simulation = Simulation::default();
        assert_eq!(simulation.tick(), 0);
        assert!(simulation.is_paused());
    }

    #[test]
    fn paused_simulation_does_not_advance_automatically() {
        let mut simulation = Simulation::default();
        assert_eq!(simulation.advance(10), 0);
    }

    #[test]
    fn resumed_simulation_advances_by_requested_ticks() {
        let mut simulation = Simulation::default();
        simulation.resume();
        assert_eq!(simulation.advance(10), 10);
        assert_eq!(simulation.advance(5), 15);
    }

    #[test]
    fn manual_step_works_while_paused() {
        let mut simulation = Simulation::default();
        assert_eq!(simulation.step(), 1);
        assert!(simulation.is_paused());
    }

    #[test]
    fn pause_stops_future_automatic_advances() {
        let mut simulation = Simulation::default();
        simulation.resume();
        simulation.advance(3);
        simulation.pause();
        simulation.advance(8);
        assert_eq!(simulation.tick(), 3);
    }
}
