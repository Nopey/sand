// TODO: Break main.rs into modules: vec, species, board, cell
use grid::Grid;
use vecmath::*;
use std::iter::Iterator;
use std::convert::TryInto;
use std::mem::swap;
use rand::random;

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

fn cap_vec(vec: Vec2) -> Vec2 {
    if vec2_square_len(vec) > 1.0 {
        vec2_normalized(vec)
    } else {
        vec
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
    Sand,
    Water,
}

impl Species {
    /// Get the visual representation
    /// of this species
    fn get_char(&self) -> char {
        match self {
            Species::Air   => ' ',
            Species::Sand  => '%',
            Species::Water => '.',
        }
    }

    /// The factor in which the cell.motion.normalized is applied to the subgrid position.
    /// range: -0.5..0.5
    fn get_subgrid_magnitude(&self) -> f32 {
        match self {
            Species::Air   => 0.0,
            Species::Sand  => 0.2,
            Species::Water => 0.4,
        }
    }

    /// How solid is this material
    /// A less solid material will let other materials
    ///     pass through and displace it more easily, like liquid.
    /// (multiplied against the mass during collisions, but not friction.)
    /// range: 0..1 ( != 0, or else NaN)
    fn get_solidity(&self) -> f32 {
        match self {
            Species::Air   => 0.1,
            Species::Sand  => 1.0,
            Species::Water => 0.7,
        }
    }

    /// How heavy is this material?
    /// Only effects collisions between particles.
    ///
    /// range: >0
    fn get_mass(&self) -> f32 {
        match self {
            Species::Air   => 0.1,
            Species::Sand  => 2.5,
            Species::Water => 1.0, // 1.0g/(cm)^3
        }
    }

    /// Acceleration that's applied to cells of this species at the beginning of the step
    /// TODO: Pass references to the Cell, Location, and Board to Species::get_gravity
    /// (that would allow for more complicated behavior)
    fn get_gravity(&self) -> Vec2 {
        match self {
            // could give a slight weight, but don't right now
            Species::Air   => [0.0,0.0],
            Species::Sand  => [0.0,1.9],
            Species::Water => [0.0,1.9],
        }
    }

    /// How much of this material's velocity is given to the neighboring cells each velocity step
    /// range: 0..1
    fn get_friction_coeff(&self) -> f32 {
        match self {
            Species::Air   => 0.20,
            Species::Sand  => 0.08,
            Species::Water => 0.05,
        }
    }

    /// How much of the energy of a collision is maintained
    /// range: 0..1
    fn get_elasticity(&self) -> f32 {
        match self {
            Species::Air   => 0.8,
            Species::Sand  => 0.0,
            Species::Water => 0.2,
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

impl Cell {
    fn get_subgrid_offset(&self) -> Vec2 {
        vec2_scale(cap_vec(self.motion), self.species.get_subgrid_magnitude())
    }
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
            //if cell.species == Species::Air {
            //    cell.velocity = vec2_scale(cell.velocity, 0.3);
            //}
        }
    }
    /// Resolves velocity
    fn velocity_step(&mut self) {
        let cols = self.grid.cols();
        for y in 0..self.grid.rows() {
            for x in 0..cols {
                let loc = [x as isize, y as isize];

                // 1. Calculate and apply pushing
                let cell = grid_get(&self.grid, loc).unwrap();
                let offset = velocity_to_offset(cell.velocity);
                let rel_pos = [offset[0] as f32, offset[1] as f32];
                let dest = vec2_add(loc, offset);
                let other = grid_get(&self.grid, dest);

                // The total momentum of the system divided by the total mass of the system = average velocity of the system.
                let our_mass = cell.species.get_mass() * cell.species.get_solidity();
                let their_mass = other.map(|o| o.species.get_mass() * o.species.get_solidity()).unwrap_or(1.0);
                let their_momentum = other.map(|o| vec2_scale(o.velocity, their_mass)).unwrap_or_default();
                let system_momentum = vec2_add(vec2_scale(cell.velocity, our_mass), their_momentum);
                let system_velocity = vec2_scale(system_momentum, 1.0/(our_mass + their_mass));

                let subgrid_offset = vec2_sub(other.map(Cell::get_subgrid_offset).unwrap_or_default(), cell.get_subgrid_offset());
                let impact_dir = vec2_normalized(vec2_add(rel_pos, subgrid_offset));

                let elasticity = cell.species.get_elasticity() * other.map(|o| o.species.get_elasticity()).unwrap_or(0.2);
                let our_impact = vec2_scale(impact_dir, vec2_dot(impact_dir, vec2_sub(cell.velocity, system_velocity)) * -(1.0 + elasticity));
                let their_impact = other.map(|other| vec2_scale(impact_dir, vec2_dot(impact_dir, vec2_sub(other.velocity, system_velocity)) * -(1.0 + elasticity))).unwrap_or_default();
                let cell = grid_get_mut(&mut self.grid, loc).unwrap();
                cell.velocity = vec2_add(cell.velocity, our_impact);
                if let Some(other) = grid_get_mut(&mut self.grid, dest) {
                    other.velocity = vec2_add(other.velocity, their_impact);
                }

                // 2. Calculate and apply friction
                let cell = grid_get_mut(&mut self.grid, loc).unwrap();
                let friction_coeff = cell.species.get_friction_coeff();
                let heat = vec2_scale(cell.velocity, friction_coeff / 4.0 * cell.species.get_mass());
                cell.velocity = vec2_scale(cell.velocity, 1.0 - friction_coeff);
                for n in neighbors(loc) {
                    if let Some(other) = grid_get_mut(&mut self.grid, n){
                        other.velocity = vec2_add(other.velocity, vec2_scale(heat, 1.0 / other.species.get_mass()));
                    }
                }
            }
        }
    }
    /// What it says on the tin
    fn copy_velocity_to_motion(&mut self) {
        for cell in self.grid.iter_mut() {
            cell.motion = vec2_add(cap_vec(cell.motion), cell.velocity);
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
    let mut board = Board::new(24, 32);
    for cell in board.grid.iter_col_mut(8) {
        cell.species = Species::Sand;
    }
    for cell in board.grid.iter_col_mut(12) {
        cell.species = Species::Water;
    }
    for cell in board.grid.iter_mut() {
        // Small perturbation, to prevent them from being perfect <3
        cell.motion = [random::<f32>() * 0.1 - 0.05, random::<f32>() * 0.1 - 0.05];
    }
    loop {
        board.print();
        board.step();
        std::thread::sleep(std::time::Duration::from_millis(50));
        println!("------");
    }
}
