use tcod::colors::*;
use tcod::console::*;
use tcod::map::{FovAlgorithm, Map as FovMap};
use std::cmp;
use rand::Rng;
use serde::{Serialize, Deserialize};
use serde_json::{Result, Value};
use std::collections::HashMap;
use std::slice::Iter;
use rand::seq::SliceRandom;

const SCREEN_WIDTH: i32 = 90;
const SCREEN_HEIGHT: i32 = 50;
const LIMIT_FPS: i32 = 20;
const MAP_WIDTH: i32 = 80;
const MAP_HEIGHT: i32 = 43;
const COLOR_DARK_WALL: Color = Color {r: 10, g: 55, b: 10};
const COLOR_LIGHT_WALL: Color = Color {r: 30, g: 100, b: 30};
const COLOR_DARK_GROUND: Color = Color {r: 5, g: 15, b: 5};
const COLOR_LIGHT_GROUND: Color = Color {r: 7,g: 35,b: 7};
const ROOM_MAX_SIZE: i32 = 10;
const ROOM_MIN_SIZE: i32 = 6;
const MAX_ROOMS: i32 = 30;
const FOV_ALGO: FovAlgorithm = FovAlgorithm::Basic;
const FOV_LIGHT_WALLS: bool = true;
const TORCH_RADIUS: i32 = 3;

const MAX_ROOM_MONSTERS: i32 = 3;

const PLAYER: usize = 0;
const GAME_DATA: &str = include_str!("data/gamedata.json");

const BAR_WIDTH: i32 = 20;
const PANEL_HEIGHT: i32 = 7;
const PANEL_Y: i32 = SCREEN_HEIGHT - PANEL_HEIGHT;


// Data Types
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
struct Point {
    x: i32,
    y: i32,
}

impl Point {
    pub fn new(x: i32, y: i32) -> Self {
        Point { x, y }
    }
}

#[derive(Debug)]
struct Object {
    x: i32,
    y: i32,
    name: String,
    character: char,
    color: Color,
    id: i32,
    blocks: bool,
    alive: bool,
    fighter: Option<Fighter>,
    ai: Option<Ai>,
}

impl Object {
    pub fn new(x: i32, y: i32, name: &str, ch: char, color: Color, id: i32, blocks: bool, alive: bool) -> Self {
        Object { x, y, name: name.into(), character: ch, color, id, blocks, alive, fighter: None, ai: None }
    }

    pub fn new_player(x: i32, y: i32, name: &str, ch: char, color: Color, id: i32, blocks: bool, alive: bool) -> Self {
        let player_fighter = Fighter::new(name);
        Object {
            x,
            y,
            name: name.into(),
            character: ch,
            color,
            id,
            blocks,
            alive,
            fighter: Some(player_fighter),
            ai: None
        }
    }

    pub fn new_monster(x: i32, y: i32, name: &str, ch: char, color: Color, id: i32, blocks: bool, alive: bool) -> Self {
        let monster_fighter = Fighter::new(name);
        Object {
            x,
            y,
            name: name.into(),
            character: ch,
            color,
            id,
            blocks,
            alive,
            fighter: Some(monster_fighter),
            ai: Some(Ai),
        }
    }

    pub fn draw(&self, con: &mut Console) {
        con.set_default_foreground(self.color);
        con.put_char(self.x, self.y, self.character, BackgroundFlag::None);
    }

    pub fn pos(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    pub fn set_pos(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
        //println!("Position: ({}, {})", self.x, self.y);
    }
    
    pub fn distance_to(&self, other: &Object) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        distance(dx, dy)
    }

    pub fn take_damage(&mut self, damage: i32) {
        if let Some(fighter) = self.fighter.as_mut() {
            if damage > 0 {
                fighter.hp -= damage;
            }
        }
        
        if let Some(fighter) = self.fighter {
            if fighter.hp <= 0 {
                self.alive = false;
                fighter.on_death.callback(self);
            }
        }
    }

    pub fn attack(&mut self, target: &mut Object) {
        let damage = self.fighter.map_or(0, |f| f.power) - target.fighter.map_or(0, |f| f.defense);
        if damage > 0 {
            println!(
                "{} attacks {} for {} hit points",
                self.name, target.name, damage
            );
            target.take_damage(damage);
        } else {
            println!(
                "{} attacks {} but it has no effect!",
                self.name, target.name
            );
        }
    }
}

enum Directions {
    NORTH,
    SOUTH,
    EAST,
    WEST,
}

impl Directions {
    pub fn iterator() -> Iter<'static, Directions> {
        static DIRECTIONS: [Directions; 4] = [Directions::NORTH, Directions::SOUTH, Directions::EAST, Directions::WEST];
        DIRECTIONS.into_iter()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PlayerAction {
    TookTurn,
    NoTurn,
    Exit,
}

#[derive(Clone, Copy, Debug)]
struct Tile {
    blocked: bool,
    block_sight: bool,
    explored: bool,
}

impl Tile {
    pub fn empty() -> Self {
        Tile{blocked: false, block_sight: false, explored: false}
    }

    pub fn wall() -> Self {
        Tile{blocked: true, block_sight: true, explored: false}
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

    pub fn center(&self) -> (i32, i32) {
        let center_x = (self.x1 + self.x2) / 2;
        let center_y = (self.y1 + self.y2) / 2;
        (center_x, center_y)
    }

    pub fn intersects_with(&self, other: &Rect) -> bool {
        (self.x1 <= other.x2) 
            && (self.x2 >= other.x1)
            && (self.y1 <= other.y2) 
            && (self.y2 >= other.y1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
struct Fighter {
    max_hp: i32,
    hp: i32,
    defense: i32,
    power: i32,
    on_death: DeathCallback
}

impl Fighter {
    pub fn new(name: &str) -> Self {
        let fighter_data = extract_node_from_gamedata(&name).unwrap();
        let fighter: Fighter = serde_json::from_value(fighter_data).unwrap();
        fighter
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
enum DeathCallback {
    Player,
    Monster,
}

impl DeathCallback {
    fn callback(self, object: &mut Object) {
        use DeathCallback::*;
        let callback: fn(&mut Object) = match self {
            Player => player_death,
            Monster => monster_death,
        };
        callback(object);
    }
}

fn extract_node_from_gamedata(node: &str) -> Result<Value> {
    let game_data: Value = serde_json::from_str(&GAME_DATA)?;
    let target_node: Value = game_data[node].clone();

    Ok(target_node)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Ai;


// Main Function
fn main() {
    let mut root = Root::initializer()
        .font("consolas12x12_gs_tc.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Rust/libtcod tutorial")
        .init();
    
    let mut con = Offscreen::new(SCREEN_WIDTH, SCREEN_HEIGHT);
    let mut panel = Offscreen::new(SCREEN_WIDTH, PANEL_HEIGHT);
    
    tcod::system::set_fps(LIMIT_FPS);
    let player = Object::new_player(0, 0, "player", '@', DARK_GREEN, 0, true, true);

    let mut objects = vec![player];
    let mut map = make_map_hauberk(&mut objects);
    //let mut map = make_map(&mut objects);

    let mut fov_map = FovMap::new(MAP_WIDTH, MAP_HEIGHT);
    create_fov(&mut fov_map, &map);

    let mut previous_player_position = (-1, -1);
    while !root.window_closed() {
        con.clear();
        let fov_recompute = previous_player_position != (objects[PLAYER].pos());
        render_all(&mut root, &mut con, &mut panel, &objects, &mut map, &mut fov_map, fov_recompute);
        root.flush();
        previous_player_position = objects[PLAYER].pos();
        let player_action = handle_keys(&mut root, &mut map, &mut objects);
        match player_action {
            PlayerAction::Exit => break,
            PlayerAction::TookTurn => {
                for id in 0..objects.len() {
                    if objects[id].ai.is_some() {
                        ai_take_turn(id, &map, &mut objects, &fov_map)
                    }
                }
            },
            PlayerAction::NoTurn => (),
        }
    }
}

fn handle_keys(root: &mut Root, map: &mut Map, objects: &mut [Object]) -> PlayerAction {
    use tcod::input::Key;
    use tcod::input::KeyCode::*;
    use PlayerAction::*;
    use Directions::*;

    let player_alive = objects[PLAYER].alive;
    let key = root.wait_for_keypress(true);
    match (key, player_alive) {
       (Key {
            code: Enter,
            alt: true,
            ..
        },
        _,
        ) => {
            // Alt+Enter: toggle fullscreen
            let fullscreen = root.is_fullscreen();
            root.set_fullscreen(!fullscreen);
            NoTurn
        }
        (Key { code: Escape, .. }, _) => return Exit,
        (Key { code: F1, .. }, _) => {
            reveal_map(map);
            NoTurn
        }
        (Key { code: Up, .. }, true) => {
            player_move_or_attack(get_direction(&NORTH), map, objects);
            TookTurn
        }
        (Key { code: Down, .. }, true) => {
            player_move_or_attack(get_direction(&SOUTH), map, objects);
            TookTurn
        },
        (Key { code: Left, .. }, true) => {
            player_move_or_attack(get_direction(&WEST), map, objects);
            TookTurn
        },
        (Key { code: Right, .. }, true) => {
            player_move_or_attack(get_direction(&EAST), map, objects);
            TookTurn
        },

        _ => NoTurn,
    }
}

// Movement Functions

fn get_direction(d: &Directions) -> (i32, i32) {
    // chooses direction
    match d {
        Directions::NORTH => (0, -1),
        Directions::SOUTH => (0, 1),
        Directions::EAST => (1, 0),
        Directions::WEST => (-1, 0),
    }
}

fn out_of_bounds(x: i32, y: i32) -> bool {
    if x > 0 && x < MAP_WIDTH && y > 0 && y < MAP_HEIGHT {
        return false
    }
    true
}

fn move_by(id: usize, (dx, dy): (i32, i32), map: &Map, objects: &mut [Object]) {
    let (x, y) = objects[id].pos();
    if !out_of_bounds(x + dx, y + dy) {
        if !is_blocked(x + dx, y + dy, map, objects) {
            objects[id].set_pos(x + dx, y + dy);
        }
    }

}

fn move_towards(id: usize, (target_x, target_y): (i32, i32), map: &Map, objects: &mut [Object]) {
    let dx = target_x - objects[id].x;
    let dy = target_y - objects[id].y;
    let distance = distance(dx, dy);

    let dx = (dx as f32 / distance).round() as i32;
    let dy = (dy as f32 / distance).round() as i32;
    move_by(id, (dx, dy), map, objects);
}

fn distance(dx: i32, dy: i32) -> f32 {
    ((dx.pow(2) + dy.pow(2)) as f32).sqrt()
}

fn is_blocked(x: i32, y: i32, map: &Map, objects: &[Object]) -> bool {
    // first test the map tile
    if map[x as usize][y as usize].blocked {
        return true;
    }
    // now check for any blocking objects
    objects
        .iter()
        .any(|object| object.blocks && object.pos() == (x, y))
}

// Player Functions

fn player_move_or_attack((dx, dy): (i32, i32), map: &Map, objects: &mut [Object]) {
    let x = objects[PLAYER].x + dx;
    let y = objects[PLAYER].y + dy;

    let target_id = objects
        .iter()
        .position(|object| object.fighter.is_some() && object.pos() == (x, y));

    match target_id {
        Some(target_id) => {
            let (player, target) = mut_two(PLAYER, target_id, objects);
            player.attack(target);
        }
        None => {
            move_by(PLAYER, (dx, dy), map, objects);
        }
    }
}

fn player_death(player: &mut Object) {
    println!("You DIED");

    player.character = '%';
    player.color = DARK_GREY;
}

// AI Functions

fn ai_take_turn(monster_id: usize, map: &Map, objects: &mut [Object], fov_map: &FovMap) {
    let (monster_x, monster_y) = objects[monster_id].pos();
    if fov_map.is_in_fov(monster_x, monster_y) {
        if objects[monster_id].distance_to(&objects[PLAYER]) >= 2.0 {
            let (player_x, player_y) = objects[PLAYER].pos();
            move_towards(monster_id, (player_x, player_y), map, objects);
        } else if objects[PLAYER].fighter.map_or(false, |f| f.hp > 0) {
            let (monster, player) = mut_two(monster_id, PLAYER, objects);
            monster.attack(player);
        }
    }
}

fn monster_death(monster: &mut Object) {
    println!("{} died!", monster.name);
    monster.character = '%';
    monster.color = DARK_GREY;
    monster.blocks = false;
    monster.fighter = None;
    monster.ai = None;
    monster.name = format!("remains of {}", monster.name);
}

// System Functions

fn mut_two<T>(first_index: usize, second_index: usize, items: &mut [T]) -> (&mut T, &mut T) {
    assert!(first_index != second_index);
    let split_at_index = cmp::max(first_index, second_index);
    let (first_slice, second_slice) = items.split_at_mut(split_at_index);
    if first_index < second_index {
        (&mut first_slice[first_index], &mut second_slice[0])
    } else {
        (&mut second_slice[0], &mut first_slice[second_index])
    }
}

// Rendering

fn render_all(root: &mut Root,
    con: &mut Offscreen, 
    panel: &mut Offscreen,
    objects: &[Object], 
    map: &mut Map,
    fov_map: &mut FovMap,
    fov_recompute: bool,
) {
    if fov_recompute {
        let player = &objects[PLAYER];
        fov_map.compute_fov(player.x, player.y, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO);
    } 
    let characters = vec!['!', '#', '$', '&', '*', '+', '/', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
                          '[', ']', '{', '}', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M',
                          'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z'];
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let visible = fov_map.is_in_fov(x, y);
            let wall = map[x as usize][y as usize].block_sight;
            let explored = &mut map[x as usize][y as usize].explored;
            if visible {
                *explored = true;
            }
            let color = match (visible, wall) {
                (false, true) => COLOR_DARK_WALL,
                (false, false) => COLOR_DARK_GROUND,
                (true, true) => COLOR_LIGHT_WALL,
                (true, false) => COLOR_LIGHT_GROUND,
            };
            if *explored {
                con.set_char_background(x, y, color, BackgroundFlag::Set);
            }
            else {
                let random_chance = rand::thread_rng().gen_range(0, 100);
                if random_chance < 10 {
                    let random_index = rand::thread_rng().gen_range(0, characters.len());
                    let chosen_char = &characters[random_index];
                    con.set_default_foreground(COLOR_DARK_WALL);
                    con.put_char(x, y, *chosen_char, BackgroundFlag::Set);
                }
            }
        }
    }

    let mut to_draw: Vec<_> = objects
        .iter()
        .filter(|o| fov_map.is_in_fov(o.x, o.y))
        .collect();
    to_draw.sort_by(|o1, o2| {o1.blocks.cmp(&o2.blocks) });
    for object in &to_draw {
        object.draw(con);
    }

    panel.set_default_background(BLACK);
    panel.clear();

    let hp = objects[PLAYER].fighter.map_or(0, |f| f.hp);
    let max_hp = objects[PLAYER].fighter.map_or(0, |f| f.max_hp);
    render_bar(
        panel,
        1,
        1,
        BAR_WIDTH,
        "HP",
        hp,
        max_hp,
        LIGHT_YELLOW,
        DARKER_YELLOW,
        DARKEST_GREY,
    );

    render_border(panel, DARKER_GREEN);

    blit(panel, (0, 0), (SCREEN_WIDTH, PANEL_HEIGHT), root, (0, PANEL_Y), 1.0, 1.0);

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

fn render_bar(
    panel: &mut Offscreen,
    x: i32,
    y: i32,
    total_width: i32,
    name: &str,
    value: i32,
    maximum: i32,
    bar_color: Color,
    back_color: Color,
    text_color: Color,
) {
    let bar_width = (value as f32 / maximum as f32 * total_width as f32) as i32;

    panel.set_default_background(back_color);
    panel.rect(x, y, total_width, 1, false, BackgroundFlag::Screen);

    panel.set_default_background(bar_color);
    if bar_width > 0 {
        panel.rect(x, y, bar_width, 1, false, BackgroundFlag::Screen);
    }

    panel.set_default_foreground(text_color);
    panel.print_ex(
        x + total_width / 2,
        y,
        BackgroundFlag::None,
        TextAlignment::Center,
        &format!("{}: {}/{}", name, value, maximum),
    );
}

fn render_border(panel: &mut Offscreen, border_color: Color) {
    panel.set_default_foreground(border_color);
    // Add the 4 corners
    panel.put_char(0, 0, 218u8 as char, BackgroundFlag::None);
    panel.put_char(MAP_WIDTH, 0, 191u8 as char, BackgroundFlag::None);
    panel.put_char(0, PANEL_HEIGHT-1, 192u8 as char, BackgroundFlag::None);
    panel.put_char(MAP_WIDTH, PANEL_HEIGHT-1, 217u8 as char, BackgroundFlag::None);

    // Draw top and bottom lines
    for x in 1..MAP_WIDTH {
        panel.put_char(x, 0, 196u8 as char, BackgroundFlag::None);
        panel.put_char(x, PANEL_HEIGHT-1, 196u8 as char, BackgroundFlag::None);
    }
    for y in 1..PANEL_HEIGHT-1 {
        panel.put_char(0, y, 179u8 as char, BackgroundFlag::None);
        panel.put_char(MAP_WIDTH, y, 179u8 as char, BackgroundFlag::None);
    }
}


// Map Functions

fn make_map(objects: &mut Vec<Object>) -> Map {
    // fill map with "blocked" tiles
    let mut map = vec![vec![Tile::wall(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];
    // map algo
    let mut rooms = vec![];
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
            if rand::random::<f32>() < 0.15 {
                create_circle_room(new_room, &mut map);
            } else {
                create_room(new_room, &mut map);
            }
            let room_center = new_room.center();
            place_objects(new_room, objects);

            if rooms.is_empty() {
                objects[PLAYER].set_pos(room_center.0, room_center.1);
            } else {
                let prev_center = rooms[rooms.len() - 1].center();
                if rand::random() {
                    create_h_tunnel(prev_center.0, room_center.0, prev_center.1, &mut map);
                    create_v_tunnel(prev_center.1, room_center.1, room_center.0, &mut map);
                } else {
                    create_v_tunnel(prev_center.1, room_center.1, prev_center.0, &mut map);
                    create_h_tunnel(prev_center.0, room_center.0, room_center.1, &mut map);
                }
            }

            rooms.push(new_room);
        }
    }

    map
}

fn create_room(room: Rect, map: &mut Map) {
    for x in (room.x1 + 1)..room.x2 {
        for y in (room.y1 + 1)..room.y2 {
            map[x as usize][y as usize] = Tile::empty();
        }
    }
}

fn create_circle_room(room: Rect, map: &mut Map) {
    // Code from @Agka on roguelikedev-help Discord channel - many thanks!
    let rdx = room.x2 - room.x1;
    let rdy = room.y2 - room.y1;

    let div_val = cmp::min(rdx, rdy);

    let radius: f32 = (div_val as f32 / 2.0) - 1.0;
    let rad_floor: i32 = radius.floor() as i32;
    let radsqr = radius.floor().powf(2.0) as i32;
    let (center_x, center_y) = room.center();

    let x_ratio = cmp::max(rdx / rdy, 1);
    let y_ratio = cmp::max(rdy / rdx, 1);

    for x in center_x - rad_floor - 1..center_x + rad_floor + 1 {
        for y in center_y - rad_floor - 1..center_y + rad_floor + 1 {
            let dx = (x - center_x) / x_ratio;
            let dy = (y - center_y) / y_ratio;
            let distsqr = dx.pow(2) + dy.pow(2);
            if distsqr < radsqr {
                map[x as usize][y as usize] = Tile::empty();
            }
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

fn place_objects(room: Rect, objects: &mut Vec<Object>) {
    let num_monsters = rand::thread_rng().gen_range(0, MAX_ROOM_MONSTERS + 1);

    for _ in 0..num_monsters {
        let x = rand::thread_rng().gen_range(room.x1 + 1, room.x2);
        let y = rand::thread_rng().gen_range(room.y1 + 1, room.y2);

        let monster = if rand::random::<f32>() < 0.8 {
            Object::new_monster(x, y, "worm", 'w', DESATURATED_GREEN, get_new_object_id(&objects), true, true)
        } else {
            Object::new_monster(x, y, "virus", 'v', DARKER_GREEN, get_new_object_id(&objects), true, true)
        };

        objects.push(monster);
    }
}

fn reveal_map(map: &mut Map) {
    for x in 0..MAP_WIDTH {
        for y in 0..MAP_HEIGHT {
            map[x as usize][y as usize].explored = true;
        }
    }
}

fn get_new_object_id(objects: &Vec<Object>) -> i32 {
    let last_item = objects.last();
    match last_item {
        Some(n) => n.id + 1,
        None => 1,
    }
}

// The Hauberk Map Generater

fn make_map_hauberk(objects: &mut Vec<Object>) -> Map {

    let num_room_tries = 100;
    let extra_connector_chance = 4;
    //let room_extra_size = 0;
    let winding_percent = 10;

    let current_region: i32 = -1;

    let mut map_width: i32 = MAP_WIDTH;
    let mut map_height: i32 = MAP_HEIGHT;

    if map_width % 2 == 0 {
        map_width -= 1;
    }

    if map_height % 2 == 0 {
        map_height -= 1;
    }

    let mut map = vec![vec![Tile::wall(); map_height as usize]; MAP_WIDTH as usize];

    type VecRegion = Vec<Vec<i32>>;

    let mut _regions = vec![vec![0; map_height as usize]; MAP_WIDTH as usize];

    //fn on_decorate_room(room: Rect) {}

    fn grow_maze(map: &mut Map, start: Point, current_region: i32, winding_percent: i32, _regions: &mut VecRegion) {
        let mut cells = Vec::new();
        let mut last_dir = (0, 0);

        start_region(current_region);
        carve(&start, map, _regions, current_region);

        cells.push(start);

        while !cells.is_empty() {
            let cell = cells.last().unwrap();

            let mut unmade_cells = Vec::new();
            for d in Directions::iterator() {
                let (dx, dy) = get_direction(d);
                let target_pos: Point = Point::new(cell.x + dx, cell.y + dy);
                if can_carve(map, target_pos, d) {
                    unmade_cells.push((dx, dy));
                }
            }

            if !unmade_cells.is_empty() {
                let mut dir = (0, 0);
                if unmade_cells.contains(&last_dir) && rand::thread_rng().gen_range(0, 100) > winding_percent {
                    dir = last_dir;
                } else {
                    let dir_choice = unmade_cells.choose(&mut rand::thread_rng());
                    match dir_choice {
                        Some(d) => {dir = *d;}
                        None => ()
                    }
                }

                let close_pos = Point::new(cell.x + dir.0, cell.y + dir.1);
                let far_pos = Point::new(cell.x + (dir.0 * 2), cell.y + (dir.1 * 2));
                carve(&close_pos, map, _regions, current_region);
                carve(&far_pos, map, _regions, current_region);

                cells.push(far_pos);

                last_dir = dir;

            } else {
                cells.pop();
                last_dir = (0, 0);
            }
        }

    }

    fn add_rooms(objects: &mut Vec<Object>, map: &mut Map, tries: i32, current_region: i32, _regions: &mut VecRegion,
                 map_width: i32, map_height: i32) {
        let mut rooms = Vec::new();
        for i in 0..=tries {
            let w = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
            let h = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
            let x = (rand::thread_rng().gen_range(0, map_width - w - 1) / 2)* 2 + 1;
            let y = (rand::thread_rng().gen_range(0, map_height - h - 1) / 2) * 2 + 1;

            let new_room = Rect::new(x, y, w, h);

            let failed = rooms
            .iter()
            .any(|other_room| new_room.intersects_with(other_room));

            if rooms.is_empty() {
                let room_center = new_room.center();
                objects[PLAYER].set_pos(room_center.0, room_center.1);
            }

            if !failed {
                place_objects(new_room, objects);
                rooms.push(new_room);
                start_region(current_region);
                create_room_hauberk(&new_room, map, _regions, current_region);
            }
        }
    }

    fn connect_regions(map: &mut Map, _regions: &mut VecRegion, current_region: i32, extra_chance: i32,
                       map_width: i32, map_height: i32) {
        let mut connector_regions = HashMap::new();

        for x in 1..map_width-1 {
            for y in 1..map_height-1 {
                if !map[x as usize][y as usize].block_sight { continue; }

                let mut regions = Vec::new();
                for d in Directions::iterator() {
                    let (dx, dy) = get_direction(d);
                    let region = _regions[(x + dx) as usize][(y + dy) as usize];
                    regions.push(region);
                }

                if regions.len() < 2 { continue; }

                connector_regions.insert(Point::new(x, y), regions);
            }
        }

        let mut connectors: Vec<_> = connector_regions.keys().collect();

        let mut merged = HashMap::new();
        let mut open_regions = Vec::new();

        for i in 0..current_region {
            merged.insert(i, i);
            open_regions.push(i);
        }

        while open_regions.len() > 1 {
            let connector = connectors.choose(&mut rand::thread_rng());
            let mut regions = Vec::new();
            match connector {
                Some(connector) => {
                    add_junction(connector, map, _regions, current_region);
                    for region in &connector_regions[connector] {
                        let actual_region = merged[&region];
                        regions.push(actual_region);
                    }
                    let dest = regions.first().unwrap().clone();
                    let sources: Vec<_> = regions[1..].iter().collect();

                    for i in 0..current_region {
                        if sources.contains(&&merged[&i]) {
                            merged.remove(&i);
                            merged.insert(i, dest);
                        }
                    }

                    for s in sources {
                        let index = open_regions.iter().position(|x| *x == *s).unwrap();
                        open_regions.remove(index);
                    }

                    let mut to_be_removed = Vec::new();
                    for pos in &connectors {
                        if distance(connector.x-pos.x, connector.y-pos.y) < 2.0 {
                            to_be_removed.push(*pos);
                            continue;
                        }

                        let mut local_regions = Vec::new();
                        for r in &connector_regions[connector] {
                            let region_actual = merged[&r];
                            local_regions.push(region_actual);
                        }

                        if local_regions.len() > 1 { continue; }

                        if rand::thread_rng().gen_range(0, 100) < extra_chance {
                            add_junction(pos, map, _regions, current_region);
                        }

                        if local_regions.len() == 1 {
                            to_be_removed.push(*pos);
                        }
                    }

                    connectors.retain(|&x| !to_be_removed.contains(&x));
                }
                None => ()
            }

        }

        
    }

    fn add_junction(pos: &Point, map: &mut Map, _regions: &mut VecRegion, current_region: i32) {
        if rand::random::<f32>() < 0.25 {
            carve(pos, map, _regions, current_region);
        }
    }

    fn remove_dead_ends(map: &mut Map, map_width: i32, map_height: i32) {
        let mut done = false;

        while !done {
            done = true;

            for x in 1..map_width {
                for y in 1..map_height {
                    if map[x as usize][y as usize].block_sight { continue; }

                    let mut exits = 0;
                    for d in Directions::iterator() {
                        let (dx, dy) = get_direction(d);
                        let (target_x, target_y) = (x + dx, y + dy);
                        if !map[target_x as usize][target_y as usize].block_sight { exits += 1; }
                    }

                    if exits != 1 { continue; }

                    done = false;
                    map[x as usize][y as usize] = Tile::wall();
                }
            }
        }

    }

    fn can_carve(map: &mut Map, pos: Point, d: &Directions) -> bool {
        let (dx, dy) = get_direction(d);
        let test_point = (pos.x + (dx*3), pos.y + (dy*3));
        if out_of_bounds(test_point.0, test_point.1) {
            return false
        }

        let (target_x, target_y) = (pos.x + dx, pos.y + dy);

        return map[target_x as usize][target_y as usize].block_sight;
    }

    fn start_region(i: i32) -> i32 {
        i + 1
    }

    fn carve(pos: &Point, map: &mut Map, _regions: &mut VecRegion, current_region: i32) {
        map[pos.x as usize][pos.y as usize] = Tile::empty();
        //println!("Made ({}, {}) a floor tile", pos.x, pos.y);
        _regions[pos.x as usize][pos.y as usize] = current_region;
    }

    fn create_room_hauberk(room: &Rect, map: &mut Map, _regions: &mut VecRegion, current_region: i32) {
        for x in room.x1..room.x2 {
            for y in room.y1..room.y2 {
                let target_point = Point::new(x, y);
                carve(&target_point, map, _regions, current_region);
            }
        }
    }

    add_rooms(objects, &mut map, num_room_tries, current_region, &mut _regions, map_width, map_height);

    for y in (1..map_height).step_by(2) {
        for x in (1..map_width).step_by(2) {
            if !map[x as usize][y as usize].block_sight { continue ; }

            let start = Point::new(x, y);
            grow_maze(&mut map, start, current_region, winding_percent, &mut _regions);
        }
    }

    connect_regions(&mut map, &mut _regions, current_region, extra_connector_chance, map_width, map_height);

    remove_dead_ends(&mut map, map_width, map_height);

    map
}