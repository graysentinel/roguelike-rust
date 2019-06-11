use tcod::colors::*;
use tcod::console::*;

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;
const LIMIT_FPS: i32 = 20;
const MAP_WIDTH: i32 = 80;
const MAP_HEIGHT: i32 = 45;
const COLOR_DARK_WALL: Color = Color { r: 10, g: 75, b: 10 };
const COLOR_DARK_GROUND: Color = Color { r: 0, g: 0, b: 0 };


// Data Types
#[derive(Debug)]
struct Point {
    x: i32,
    y: i32,
}

impl Point {
    pub fn new(x: i32, y: i32) -> Self {
        Point {x, y}
    }
}

#[derive(Debug)]
struct Object {
    character: char,
    position: Point,
    color: Color,
}

impl Object {
    pub fn new(pos: Point, ch: char, color: Color) -> Self {
        Object { character: ch, position: pos, color }
    }

    pub fn draw(&self, con: &mut Console) {
        con.set_default_foreground(self.color);
        con.put_char(self.position.x, self.position.y, self.character, BackgroundFlag::None);
    }

    pub fn move_d(&mut self, d: Directions, map: &Map) {
        let tgt_pos = get_direction(self.position.x, self.position.y, d);
        if self.can_move(&tgt_pos, map) {
            self.position = tgt_pos;
        }

    }

    pub fn can_move(&self, pos: &Point, map: &Map) -> bool {
        if !out_of_bounds(&pos) {
            if !map[pos.x as usize][pos.y as usize].blocked {
                return true
            }
        }

        false
    }
}

enum Directions {
    NORTH,
    SOUTH,
    EAST,
    WEST,
}

#[derive(Clone, Copy, Debug)]
struct Tile {
    blocked: bool,
    block_sight: bool,
}

impl Tile {
    pub fn empty() -> Self {
        Tile{blocked: false, block_sight: false}
    }

    pub fn wall() -> Self {
        Tile{blocked: true, block_sight: true}
    }
}

type Map = Vec<Vec<Tile>>;


// Main Function
fn main() {
    let mut root = Root::initializer()
        .font("terminal10x10.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Rust/libtcod tutorial")
        .init();
    
    let mut con = Offscreen::new(SCREEN_WIDTH, SCREEN_HEIGHT);
    
    tcod::system::set_fps(LIMIT_FPS);

    let start_pos = Point{x: SCREEN_WIDTH / 2, y: SCREEN_HEIGHT / 2};
    let player = Object::new(start_pos, '@', WHITE);
    let npc = Object::new(Point::new(SCREEN_WIDTH/2-5, SCREEN_HEIGHT/2), '@', YELLOW);
    let mut objects = [player, npc];
    let map = make_map();

    while !root.window_closed() {
        con.set_default_foreground(WHITE);
        con.clear();
        render_all(&mut root, &mut con, &objects, &map);
        root.flush();
        let player = &mut objects[0];
        let exit = handle_keys(&mut root, player, &map);
        if exit {
            break
        }
    }
}


// Subroutines
fn handle_keys(root: &mut Root, player: &mut Object, map: &Map) -> bool {
    use tcod::input::Key;
    use tcod::input::KeyCode::*;

    let key = root.wait_for_keypress(true);
    match key {
        Key {
            code: Enter,
            alt: true,
            ..
        } => {
            // Alt+Enter: toggle fullscreen
            let fullscreen = root.is_fullscreen();
            root.set_fullscreen(!fullscreen)
        }
        Key { code: Escape, .. } => return true,
        Key { code: Up, .. } => {
            Object::move_d(player, Directions::NORTH, map);
        }
        Key { code: Down, .. } => {
            Object::move_d(player, Directions::SOUTH, map);
        },
        Key { code: Left, .. } => {
            Object::move_d(player, Directions::WEST, map);
        },
        Key { code: Right, .. } => {
            Object::move_d(player, Directions::EAST, map);
            },

        _ => {},
    }

    false
}

fn get_direction(x: i32, y: i32, d: Directions) -> Point {
    // chooses direction and calculates target point
    match d {
        Directions::NORTH => Point::new(x, y - 1),
        Directions::SOUTH => Point::new(x, y + 1),
        Directions::EAST => Point::new(x + 1, y),
        Directions::WEST => Point::new(x - 1, y),
    }
}

fn make_map() -> Map {
    // fill map with "unblocked" tiles
    let mut map = vec![vec![Tile::empty(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];
    map[30][22] = Tile::wall();
    map[50][22] = Tile::wall();

    map
}

fn out_of_bounds(pos: &Point) -> bool {
    if pos.x < MAP_WIDTH && pos.y < MAP_HEIGHT {
        return false
    }

    true
}

fn render_all(root: &mut Root, con: &mut Offscreen, objects: &[Object], map: &Map) {
    //draw all objects in the list
    for object in objects {
        object.draw(con);
    }

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let wall = map[x as usize][y as usize].block_sight;
            if wall {
                con.set_char_background(x, y, COLOR_DARK_WALL, BackgroundFlag::Set);
            } else {
                con.set_char_background(x, y, COLOR_DARK_GROUND, BackgroundFlag::Set);
            }
        }
    }

    blit(con, (0, 0), (MAP_WIDTH, MAP_HEIGHT), root, (0, 0), 1.0, 1.0);
}