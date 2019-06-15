use tcod::colors::*;
use tcod::console::*;
use tcod::map::{FovAlgorithm, Map as FovMap};
use std::cmp;
use rand::Rng;

const SCREEN_WIDTH: i32 = 90;
const SCREEN_HEIGHT: i32 = 50;
const LIMIT_FPS: i32 = 20;
const MAP_WIDTH: i32 = 80;
const MAP_HEIGHT: i32 = 45;
const COLOR_DARK_WALL: Color = Color { r: 10, g: 55, b: 10 };
const COLOR_LIGHT_WALL: Color = Color {
    r: 30, g: 100, b: 30
};
const COLOR_DARK_GROUND: Color = Color { r: 5, g: 15, b: 5 };
const COLOR_LIGHT_GROUND: Color = Color {
    r: 7,
    g: 35,
    b: 7
};
const ROOM_MAX_SIZE: i32 = 10;
const ROOM_MIN_SIZE: i32 = 6;
const MAX_ROOMS: i32 = 30;
const FOV_ALGO: FovAlgorithm = FovAlgorithm::Basic;
const FOV_LIGHT_WALLS: bool = true;
const TORCH_RADIUS: i32 = 3;


// Data Types
#[derive(Debug, PartialEq, Copy, Clone)]
struct Point {
    x: i32,
    y: i32,
}

impl Point {
    pub fn new(x: i32, y: i32) -> Self {
        Point {x, y}
    }
}

#[derive(Debug, Copy, Clone)]
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

#[derive(Clone, Copy, Debug)]
struct Rect {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Rect {
            x1: x,
            y1: y,
            x2: x + w,
            y2: y + h,
        }
    }

    pub fn center(&self) -> Point {
        let center_x = (self.x1 + self.x2) / 2;
        let center_y = (self.y1 + self.y2) / 2;
        Point::new(center_x, center_y)
    }

    pub fn intersects_with(&self, other: &Rect) -> bool {
        (self.x1 <= other.x2) 
            && (self.x2 >= other.x1)
            && (self.y1 <= other.y2) 
            && (self.y2 >= other.y1)
    }
}


// Main Function
fn main() {
    let mut root = Root::initializer()
        .font("consolas12x12_gs_tc.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Rust/libtcod tutorial")
        .init();
    
    let mut con = Offscreen::new(SCREEN_WIDTH, SCREEN_HEIGHT);
    
    tcod::system::set_fps(LIMIT_FPS);
    let (map, start_pos) = make_map();
    let player = Object::new(start_pos, '@', WHITE);
    let mut objects = [player];

    let mut fov_map = FovMap::new(MAP_WIDTH, MAP_HEIGHT);
    create_fov(&mut fov_map, &map);

    let mut previous_player_position = Point::new(-1, -1);
    while !root.window_closed() {
        con.set_default_foreground(WHITE);
        con.clear();
        let fov_recompute = previous_player_position != player.position;
        render_all(&mut root, &mut con, &objects, &map, &mut fov_map, fov_recompute);
        root.flush();
        let player = &mut objects[0];
        previous_player_position = Point::new(player.position.x, player.position.y);
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

fn out_of_bounds(pos: &Point) -> bool {
    if pos.x < MAP_WIDTH && pos.y < MAP_HEIGHT {
        return false
    }

    true
}

fn render_all(root: &mut Root, 
    con: &mut Offscreen, 
    objects: &[Object], 
    map: &Map,
    fov_map: &mut FovMap,
    fov_recompute: bool,
) {
    if fov_recompute {
        let player = &objects[0];
        fov_map.compute_fov(player.position.x, player.position.y, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO);
    }
    //draw all objects in the list

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let visible = fov_map.is_in_fov(x, y);
            let wall = map[x as usize][y as usize].block_sight;
            let color = match (visible, wall) {
                (false, true) => COLOR_DARK_WALL,
                (false, false) => COLOR_DARK_GROUND,
                (true, true) => COLOR_LIGHT_WALL,
                (true, false) => COLOR_LIGHT_GROUND,
            };
            con.set_char_background(x, y, color, BackgroundFlag::Set);
        }
    }

    for object in objects {
        if fov_map.is_in_fov(object.position.x, object.position.y) {
            object.draw(con);
        }
    }

    blit(con, (0, 0), (MAP_WIDTH, MAP_HEIGHT), root, (0, 0), 1.0, 1.0);
}

fn create_fov(fov: &mut FovMap, map: &Map) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            fov.set(
                x,
                y,
                !map[x as usize][y as usize].block_sight,
                !map[x as usize][y as usize].blocked,
            );
        }
    }
}

// Map Functions

fn make_map() -> (Map, Point) {
    // fill map with "blocked" tiles
    let mut map = vec![vec![Tile::wall(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];
    // map algo
    let mut rooms = vec![];
    let mut start_pos = Point::new(0, 0);
    for _ in 0..MAX_ROOMS {
        let w = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
        let h = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
        let x = rand::thread_rng().gen_range(0, MAP_WIDTH - w);
        let y = rand::thread_rng().gen_range(0, MAP_HEIGHT - h);

        let new_room = Rect::new(x, y, w, h);

        let failed = rooms
            .iter()
            .any(|other_room| new_room.intersects_with(other_room));
        
        if !failed {
            create_room(new_room, &mut map);
            let room_center = new_room.center();

            if rooms.is_empty() {
                start_pos = Point::new(room_center.x, room_center.y);
            } else {
                let prev_center = rooms[rooms.len() - 1].center();
                if rand::random() {
                    create_h_tunnel(prev_center.x, room_center.x, prev_center.y, &mut map);
                    create_v_tunnel(prev_center.y, room_center.y, room_center.x, &mut map);
                } else {
                    create_v_tunnel(prev_center.y, room_center.y, prev_center.x, &mut map);
                    create_h_tunnel(prev_center.x, room_center.x, room_center.y, &mut map);
                }
            }

            rooms.push(new_room);
        }
    }

    (map, start_pos)
}

fn create_room(room: Rect, map: &mut Map) {
    for x in (room.x1 + 1)..room.x2 {
        for y in (room.y1 + 1)..room.y2 {
            map[x as usize][y as usize] = Tile::empty();
        }
    }
}

fn create_h_tunnel(x1: i32, x2: i32, y: i32, map: &mut Map) {
    for x in cmp::min(x1, x2)..(cmp::max(x1, x2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn create_v_tunnel(y1: i32, y2: i32, x: i32, map: &mut Map) {
    for y in cmp::min(y1, y2)..(cmp::max(y1, y2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}