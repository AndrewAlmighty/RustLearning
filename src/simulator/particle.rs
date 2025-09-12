pub(super) enum Type {
    Light,
    Normal,
    Heavy,
    Supermassive
}

pub(super) struct Particle {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    mass: f64
}

impl Particle {
    pub(super) fn create(x: f64, y: f64, mass: f64) -> Self {
        Particle { x: x, y: y, mass: mass, vx: 0.0, vy: 0.0 }
    }

    pub(super) fn get_current_position(&self) -> (f64, f64) {
        (self.x, self.y)
    }

    pub(super) fn get_current_position_for_display(&self) -> (usize, usize) {
        (self.x.round() as usize, self.y.round() as usize)
    }

    pub(super) fn get_mass(&self) -> f64 {
        self.mass
    }

    pub(super) fn get_velocity(&self) -> (f64, f64) {
        (self.vx, self.vy)
    }

    pub(super) fn update_velocity(&mut self, fx: f64, fy: f64, dt: f64) {
        let ax = fx / self.mass;
        let ay = fy / self.mass;

        self.vx += ax * dt;
        self.vy += ay * dt;
    }

    pub(super) fn move_particle(&mut self, dt: f64) {
        self.x += self.vx * dt;
        self.y += self.vy * dt;
    }

    pub(super) fn absort_particle(&mut self, other_particle_mass: f64, other_particle_velocity: (f64, f64)) {
        let total_mass = self.mass + other_particle_mass;
        self.vx = (self.vx * self.mass + other_particle_velocity.0 * other_particle_mass) / total_mass;
        self.vy = (self.vy * self.mass + other_particle_velocity.1 * other_particle_mass) / total_mass;
        self.mass = total_mass;
    }

    pub(super) fn get_particle_char(&self) -> char {
        match self.mass {
            1.0..=10.0 => '.',
            11.0..=50.0 => '*',
            51.0..=100.0 => '#',
            101.0.. => '@',
            mass => { panic!("There is a particle with mass: {}", mass); }
        }
    }
}