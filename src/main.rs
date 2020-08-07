// TODO: Disallow unused code
// TODO: Break main.rs into modules: vec, species, board, cell
#![allow(unused)]
use grid::Grid;
use std::ops::{Add, AddAssign};
use vecmath::*;
use std::iter::Iterator;
use std::convert::TryInto;
use std::mem::swap;

type Vec2 = Vector2<f32>;
type Loc2 = Vector2<isize>;

fn neighbors(loc: Loc2) -> impl Iterator<Item = Loc2> {
    [
        [1, 0],
        [0, 1],
        [-1, 0],
        [0, -1]
    ].iter().map(
        move |&o| vec2_add(o, loc)
    )
}

fn velocity_to_offset(vel: Vec2) -> Loc2 {
    if f32::abs(vel[0]) > f32::abs(vel[1]) {
        if vel[0] > 0.0 { [1, 0] }
        else { [-1, 0] }
    } else {
        if vel[1] > 0.0 { [0, 1] }
        else { [0, -1] }
    }
}

fn grid_get<T>(grid: &Grid<T>, loc: Loc2) -> Option<&T>
  where T: std::clone::Clone
{    
    grid.get(loc[1].try_into().ok()?, loc[0].try_into().ok()?)
}

fn grid_get_mut<T>(grid: &mut Grid<T>, loc: Loc2) -> Option<&mut T>
  where T: std::clone::Clone
{
    grid.get_mut(loc[1].try_into().ok()?, loc[0].try_into().ok()?)
}
#[derive(Debug, Clone, Copy, PartialEq)]
enum Species {
    Air,
    Sand
}

impl Species {
    /// Get the visual representation
    /// of this species
    fn get_char(&self) -> char {
        match self {
            Species::Air  => ' ',
            Species::Sand => '%',
        }
    }

    /// Get the offset within the subgrid,
    /// x and y in the range of (-0.5)..(0.5)
    ///
    /// The result may depend on random number generation,
    /// thus varying from call to call.
    // TODO: Species::get_subgrid_offset: Replace with cell.motion.normalized * cell.species.get_subgrid_magnitude?
    fn get_subgrid_offset(&self) -> Vec2 {
        use rand::random;
        match self {
            Species::Air  => [0.0, 0.0],
            Species::Sand => [random::<f32>() * 0.5 - 0.25, random::<f32>() * 0.5 - 0.25],
        }
    }

    /// How liquid is this material? (rather than solid)
    /// A less solid material will let other materials
    ///     pass through and displace it more easily.
    ///
    /// range: 0..1
    fn get_fluidity(&self) -> f32 {
        match self {
            Species::Air  => 0.8, // some thick air in here, eh?
            Species::Sand => 0.2,
        }
    }

    /// How heavy is this material?
    /// Only effects collisions between particles.
    ///
    /// range: >0
    fn get_mass(&self) -> f32 {
        match self {
            Species::Air  => 0.1,
            Species::Sand => 1.0,
        }
    }

    /// Acceleration that's applied to cells of this species at the beginning of the step
    /// TODO: Pass references to the Cell, Location, and Board to Species::get_gravity
    /// (that would allow for more complicated behavior)
    fn get_gravity(&self) -> Vec2 {
        match self {
            // could give a slight weight, but don't right now
            Species::Air  => [0.0,0.0],
            Species::Sand => [0.0,1.9],
        }
    }

    /// How much of this material's velocity is given to the neighboring cells each velocity step
    /// range: 0..1
    fn get_friction_coeff(&self) -> f32 {
        match self {
            Species::Air  => 0.16,
            Species::Sand => 0.05,
        }
    }
}

impl Default for Species {
    fn default() -> Self {
        Species::Air
    }
}

#[derive(Debug, Clone, Default)]
struct Cell {
    /// Velocity is maintained over multiple steps
    velocity: Vec2,
    /// Motion is set to velocity at the start of the step, and depleted by the end of a step
    motion: Vec2,
    species: Species,
}

#[derive(Debug, Clone)]
pub struct Board {
    grid: Grid<Cell>,
}

// TODO: Board: have functions to get and set and such.
// TODO: Board: have functions to initialize with some random sand, random subgrid motion, etc.
impl Board {
    /// Creates an empty board
    pub fn new(cols: usize, rows: usize) -> Self {
        let grid = Grid::new(rows, cols);
        Board{
            grid,
        }
    }

    /// Prints the board to stdout
    pub fn print(&self) {
        let mut x = 0;
        let maxx = self.grid.cols() - 1;
        for cell in self.grid.iter() {
            print!("{}", cell.species.get_char());
            if x == maxx {
                println!("");
                x = 0;
            } else {
                x += 1;
            }
        }
    }
    /// Single steps the simulation forward.
    pub fn step(&mut self) {
        self.gravity_step();
        for _ in 0..4 {
            self.velocity_step();
        }
        self.copy_velocity_to_motion();
        for _ in 0..10 {
            if self.motion_step() { break; }
        }
    }
    fn gravity_step(&mut self) {
        // This is related to the TODO on Species::get_gravity
        // Doing that would require seperating the calculation
        //   and application of gravity into two loops.
        for cell in self.grid.iter_mut() {
            cell.velocity = vec2_add(cell.velocity, cell.species.get_gravity());
            //HACKHACK: Try to calm the sandstorm by slowing down the air
            if cell.species == Species::Air {
                cell.velocity = vec2_scale(cell.velocity, 0.3);
            }
        }
    }
    /// Resolves velocity
    fn velocity_step(&mut self) {
        let cols = self.grid.cols();
        for y in 0..self.grid.rows() {
            for x in 0..cols {
                let loc = [x as isize, y as isize];

                // 1. Calculate and apply friction
                let cell = grid_get_mut(&mut self.grid, loc).unwrap();
                let friction_coeff = cell.species.get_friction_coeff();
                let heat = vec2_scale(cell.velocity, friction_coeff / 4.0 * cell.species.get_mass());
                cell.velocity = vec2_scale(cell.velocity, 1.0 - friction_coeff);
                for n in neighbors(loc) {
                    if let Some(other) = grid_get_mut(&mut self.grid, n){
                        other.velocity = vec2_add(other.velocity, vec2_scale(heat, 1.0 / other.species.get_mass()));
                    }
                }

                // 2. Calculate and apply pushing
                let cell = grid_get(&self.grid, loc).unwrap();
                let offset = velocity_to_offset(cell.velocity);
                let rel_pos = [offset[0] as f32, offset[1] as f32];
                let dest = vec2_add(loc, offset);
                let other = grid_get(&self.grid, dest);
                let fluidity = cell.species.get_fluidity() * other.map(|c| c.species.get_fluidity()).unwrap_or_default();

                // TODO: Board::velocity_step: Did I calculate elastic collisions correctly?
                let rel_vel = vec2_sub(cell.velocity, other.map_or([0.0, 0.0], |o| o.velocity));
                // let rel_impact = vec2_mul(rel_vel, vec2_normalized(vec2_add(rel_pos, vec2_sub(other.map_or([0.0, 0.0], |o| o.species.get_subgrid_offset()), cell.species.get_subgrid_offset()))));
                let impact_dir = vec2_normalized(vec2_add(rel_pos, vec2_sub(other.map_or([0.0, 0.0], |o| o.species.get_subgrid_offset()), cell.species.get_subgrid_offset())));
                let rel_impact = vec2_scale(impact_dir, vec2_dot(impact_dir, rel_vel));
                let our_impact = vec2_scale(rel_impact, -0.5);
                let their_impact = other.map(|other| vec2_scale(rel_impact, 0.5 * cell.species.get_mass() / other.species.get_mass())).unwrap_or_default();
                let cell = grid_get_mut(&mut self.grid, loc).unwrap();
                cell.velocity = vec2_add(cell.velocity, our_impact);
                if let Some(other) = grid_get_mut(&mut self.grid, dest) {
                    other.velocity = vec2_add(other.velocity, their_impact);
                }
            }
        }
    }
    /// What it says on the tin
    fn copy_velocity_to_motion(&mut self) {
        for cell in self.grid.iter_mut() {
            // TODO: Board::copy_velocity_to_motion: consider using += instead of =, maintaining last step's leftover motion.
            cell.motion = cell.velocity;
            //HACKHACK: Try to calm the sandstorm by removing all motion from air
            if cell.species == Species::Air {
                cell.motion = Default::default();
            }
        }
    }
    /// Moves cells.
    /// If no motion occured, then this function returns false.
    /// Some motion may still need to occur, after this function is called.
    /// Repeatedly call this until it returns true. (perhaps with an upper bound)
    /// It must return true after some finite number of steps.
    fn motion_step(&mut self) -> bool {
        let cols = self.grid.cols();
        // Has no work been done yet?
        let mut no_work_done = true;
        for y in 0..self.grid.rows() {
            for x in 0..cols {
                let loc = [x as isize, y as isize];
                let cell = grid_get(&self.grid, loc).unwrap();
                if vec2_square_len(cell.motion) < 1.0 { continue }
                no_work_done = false;
                let offset = velocity_to_offset(cell.motion);
                let rel_pos = [offset[0] as f32, offset[1] as f32];
                let dest = vec2_add(loc, offset);
                let cell = grid_get_mut(&mut self.grid, loc).unwrap();
                cell.motion = vec2_sub(cell.motion, rel_pos);
                let mut storage = Default::default();
                swap(&mut storage, cell);
                if let Some(other) = grid_get_mut(&mut self.grid, dest) {
                    swap(&mut storage, other);
                }
                let cell = grid_get_mut(&mut self.grid, loc).unwrap();
                swap(&mut storage, cell);
            }
        }
        no_work_done
    }
}

fn main() {
    let mut board = Board::new(16, 16);
    for cell in board.grid.iter_col_mut(2) {
        cell.species = Species::Sand;
    }
    loop {
        board.print();
        board.step();
        std::thread::sleep_ms(100);
        println!("------");
    }
}
