use crate::simulator::particle::{Particle, Type};

use rand::prelude::*;
use rand::seq::SliceRandom;
use rand::distr::weighted::WeightedIndex;

use rayon::prelude::*;


pub struct Engine {
    galaxy: Vec<Particle>,
    galaxy_width: usize,
    galaxy_height: usize,
    galaxy_positions_count: usize,
    gravity_strength: f64,
    time_step: f64,
    display_frame: Vec<char>
}

impl Engine {
    pub fn create(galaxy_width: usize, galaxy_height: usize, particles_count: usize, gravity_strength: f64, time_step: f64) -> Result<Self, String> {
        let galaxy_positions_count = galaxy_height * galaxy_width;

        if particles_count == 0 {
            return Err("Expected at least 1 particle".to_string());
        }
        else if particles_count > galaxy_positions_count {
            return Err(format!("Particles count: {} exceeds display size: {}", particles_count, galaxy_positions_count));
        }

        let mut galaxy = Vec::with_capacity(particles_count);
        let mut rng = rand::rng();
        let choices = [Type::Light, Type::Normal, Type::Heavy, Type::Supermassive];
        let weights = [70, 20, 9, 1];
        let distribution = {
            match WeightedIndex::new(&weights) {
                Ok(wi) => wi,
                Err(e) => { return Err(format!("Error when creating weight distribution: {}", e)); }
            }
        };

        let mut available_positions = (0..galaxy_positions_count).collect::<Vec<usize>>();
        available_positions.shuffle(&mut rng);

        let mut display_frame = vec![' '; galaxy_positions_count];

        for _ in 0..particles_count {
            let mass = {
                match choices[distribution.sample(&mut rng)] {
                    Type::Light => rng.random_range(1..=10) as f64,
                    Type::Normal => rng.random_range(11..=50) as f64,
                    Type::Heavy => rng.random_range(51..=100) as f64,
                    Type::Supermassive => rng.random_range(101..=200) as f64,
                }
            };

            let pos = available_positions.pop().unwrap();
            let particle = Particle::create((pos % galaxy_width) as f64, (pos / galaxy_width) as f64, mass);
            display_frame[pos] = particle.get_particle_char();
            galaxy.push(particle);
        }

        Ok(Engine {
            galaxy_positions_count: galaxy_positions_count,
            galaxy_width: galaxy_width,
            galaxy_height: galaxy_height,
            galaxy: galaxy,
            gravity_strength: gravity_strength,
            time_step: time_step,
            display_frame: display_frame
        })
    }

    pub fn update(&mut self) {
        self.handle_collisions();
        self.calculate_particles_velocities();
        self.move_particles();
    }

    pub fn get_display_frame(&mut self) -> &Vec<char> {
        &self.display_frame
    }

    fn calculate_particles_velocities(&mut self) {
        let forces_to_apply_on_particles: Vec<_> =
            self.galaxy.par_iter().map(|p| {
                let (px, py) = p.get_current_position();
                let pm = p.get_mass();
                let mut fx_total = 0.0;
                let mut fy_total = 0.0;

                for another_p in &self.galaxy {
                    if std::ptr::eq(p, another_p) { continue; } // skip self

                    let (apx, apy) = another_p.get_current_position();
                    let apm = another_p.get_mass();

                    let dx = apx - px;
                    let dy = apy - py;
                    let eps = 1e-3;
                    let r2 = (dx * dx) + (dy * dy) + eps;
                    let r = r2.sqrt();

                    let f = (self.gravity_strength * pm * apm) / r2;
                    fx_total += f * (dx / r);
                    fy_total += f * (dy / r);
                }

                (fx_total, fy_total)
            }).collect();

        for i in 0..self.galaxy.len() {
            let (fx, fy) = forces_to_apply_on_particles[i];
            self.galaxy[i].update_velocity(fx, fy, self.time_step);
        }
    }

    fn move_particles(&mut self) {
        let galaxy_width = self.galaxy_width - 1;
        let galaxy_height = self.galaxy_height - 1;
        let positions_in_frame_to_update:Vec<(usize, usize, char)> = self.galaxy.par_iter_mut().filter_map(|p| {
            let (mut old_x, mut old_y) = p.get_current_position_for_display();
            old_x = old_x.min(galaxy_width);
            old_y = old_y.min(galaxy_height);
            p.move_particle(self.time_step);
            let (mut new_x, mut new_y) = p.get_current_position_for_display();
            new_x = new_x.min(galaxy_width);
            new_y = new_y.min(galaxy_height);

            if old_x != new_x || old_y != new_y {
                let old_position_in_frame = (old_y * self.galaxy_width) + old_x;
                let new_position_in_frame = (new_y * self.galaxy_width) + new_x;
                assert!(old_position_in_frame < self.galaxy_positions_count);
                assert!(new_position_in_frame < self.galaxy_positions_count);
                Some((old_position_in_frame, new_position_in_frame, p.get_particle_char()))
            }
            else {
                None
            }
        }).collect();

        for (old_pos, new_pos, p_char) in positions_in_frame_to_update {
            self.display_frame[old_pos] = ' ';
            self.display_frame[new_pos] = p_char;
        }
    }

    fn handle_collisions(&mut self) {
        let mut particles_to_remove = vec![false; self.galaxy.len()];
        let previous_positions: Vec<(f64, f64)> = self.galaxy.iter().map(|p| p.get_current_position()).collect();

        for i in 0..self.galaxy.len() {
            if particles_to_remove[i] { continue; }

            let (px, py) = previous_positions[i];
            let pm = self.galaxy[i].get_mass();
            let (pvx, pvy) = self.galaxy[i].get_velocity();

            for j in (i + 1)..self.galaxy.len() {
                if particles_to_remove[j] { continue; }

                let (apx, apy) = previous_positions[j];
                let apm = self.galaxy[j].get_mass();
                let (apvx, apvy) = self.galaxy[j].get_velocity();

                let dx = apx - px;
                let dy = apy - py;
                let r2 = dx * dx + dy * dy;

                let max_displacement = (pvx.powi(2) + pvy.powi(2)).sqrt() * self.time_step + (apvx.powi(2) + apvy.powi(2)).sqrt() * self.time_step;
                let collision_radius = 0.5_f64.max(max_displacement);

                if r2 < collision_radius * collision_radius {
                    let (survivor, absorbed) = if pm >= apm { (i, j) } else { (j, i) };
                    let absorbed_particle = &self.galaxy[absorbed];
                    {
                        let (mut x, mut y) = absorbed_particle.get_current_position_for_display();
                        x = x.min(self.galaxy_width - 1);
                        y = y.min(self.galaxy_height - 1);
                        let position_in_frame = (y * self.galaxy_width) + x;
                        self.display_frame[position_in_frame] = ' ';
                    }

                    let absorbed_mass = absorbed_particle.get_mass();
                    let absorbed_velocity = absorbed_particle.get_velocity();
                    self.galaxy[survivor].absort_particle(absorbed_mass, absorbed_velocity);
                    let (mut x, mut y) = self.galaxy[survivor].get_current_position_for_display();
                    x = x.min(self.galaxy_width - 1);
                    y = y.min(self.galaxy_height - 1);
                    let position_in_frame = (y * self.galaxy_width) + x;
                    self.display_frame[position_in_frame] = self.galaxy[survivor].get_particle_char();

                    particles_to_remove[absorbed] = true;
                }
            }
        }

        for idx in (0..self.galaxy.len()).rev() {
            if particles_to_remove[idx] {
                self.galaxy.remove(idx);
            }
        }
    }
}