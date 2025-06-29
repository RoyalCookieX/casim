struct World {
    size: vec2<u32>,
}

struct Cursor {
    enabled: u32,
    radius: u32,
    position: vec2<u32>,
    cell_id: u32,
}

struct Cell {
    id: u32,
    state: u32,
}

struct Push {
    local_offset: vec2<u32>,
    state: u32,
}

const CELL_ID_VOID: u32 = 0x00u;
const CELL_ID_ROCK: u32 = 0x01u;
const CELL_ID_SAND: u32 = 0x02u;
const CELL_ID_WATER: u32 = 0x03u;

@group(0) @binding(0)
var<uniform> world: World;
@group(0) @binding(1)
var<uniform> cursor: Cursor;
@group(0) @binding(2)
var<storage, read_write> cells_input: array<Cell>;
@group(0) @binding(3)
var<storage, read_write> cells_output: array<Cell>;
var<push_constant> push: Push;

fn hash_u32(value: u32) -> u32 {
    var x = value;
    x += (x << 10u);
    x ^= (x >> 6u);
    x += (x << 3u);
    x ^= (x >> 11u);
    x += (x << 15u);
    return x;
}

fn hash_vec2_u32(value: vec2<u32>) -> u32 {
    return hash_u32(value.x ^ hash_u32(value.y));
}

fn world_contains(position: vec2<u32>) -> bool {
    return position.x < world.size.x && position.y < world.size.y;
}

fn cursor_squared_distance(position: vec2<u32>) -> u32 {
    let displacement = cursor.position - position;
    return displacement.x * displacement.x + displacement.y * displacement.y;
}

fn cursor_contains(position: vec2<u32>) -> bool {
    return cursor_squared_distance(position) < cursor.radius * cursor.radius;
}

fn cell_index(position: vec2<u32>) -> u32 {
    return position.y * world.size.x + position.x;
}

@compute @workgroup_size(1, 1, 1)
fn compute_cursor(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let position = global_id.xy;
    if !world_contains(position) || cursor.enabled == 0u || !cursor_contains(position) {
        return;
    }
    let state = hash_vec2_u32(cursor.position) ^ hash_vec2_u32(position);
    cells_output[cell_index(position)] = Cell(cursor.cell_id, state);
}

fn cell_compare(position: vec2<u32>, id: u32) -> bool {
    let index = cell_index(position);
    return cells_input[index].id == id && cells_output[index].id == id;
}

fn cell_swap(from_position: vec2<u32>, to_position: vec2<u32>) {
    let from_index = cell_index(from_position);
    var s = push.state;
    var from_cell = cells_input[from_index];
    s ^= hash_vec2_u32(to_position);
    from_cell.state ^= s;
    let to_index = cell_index(to_position);
    var to_cell = cells_input[to_index];
    s ^= hash_vec2_u32(from_position);
    to_cell.state ^= s;
    cells_output[to_index] = from_cell;
    cells_output[from_index] = to_cell;
}

@compute @workgroup_size(1, 1, 1)
fn compute_step(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let position = (global_id.xy * vec2(3u)) + push.local_offset;
    if !world_contains(position) {
        return;
    }
    let index = cell_index(position);
    let id = cells_input[index].id;
    var state = push.state;
    if id == CELL_ID_VOID {
        return;
    }
    else if id == CELL_ID_SAND {
        let fall_down_position = position - vec2(0u, 1u);
        var fall_positions = array(
            fall_down_position,
            fall_down_position - vec2(1u, 0u),
            fall_down_position,
            fall_down_position + vec2(1u, 0u),
            fall_down_position,
        );
        let fall_positions_array_length = 5u;
        let fall_index_offset = state % fall_positions_array_length;
        for (var i = 0u; i < fall_positions_array_length; i++) {
            let fall_adjacent_position = fall_positions[(i + fall_index_offset) % fall_positions_array_length];
            if !world_contains(fall_adjacent_position) {
                continue;
            }
            else if cell_compare(fall_adjacent_position, CELL_ID_VOID) {
                cell_swap(position, fall_adjacent_position);
                return;
            }
        }
    }
    cells_output[index] = Cell(id, state);
}

struct Vertex {
    position: vec2<f32>,
    texture_coord: vec2<f32>,
}

struct VertToFrag {
    @builtin(position) position: vec4<f32>,
    @location(0) texture_coord: vec2<f32>,
}

var<private> vertices: array<Vertex, 3> = array(
    Vertex(vec2(-1.0, -1.0), vec2(0.0, 0.0)),
    Vertex(vec2(3.0, -1.0), vec2(2.0, 0.0)),
    Vertex(vec2(-1.0, 3.0), vec2(0.0, 2.0)),
);

@vertex
fn vertex_main(@builtin(vertex_index) vertex_index: u32) -> VertToFrag {
    let vertex = vertices[vertex_index];
    return VertToFrag(
        vec4(vertex.position, 0.0, 1.0),
        vertex.texture_coord,
    );
}

var<private> cell_colors: array<vec3<f32>, 4> = array<vec3<f32>, 4>(
    vec3(0.0, 0.0, 0.0),
    vec3(0.4, 0.4, 0.4),
    vec3(0.91, 0.773, 0.498),
    vec3(0.0, 0.0, 1.0),
);

@fragment
fn fragment_main(vert_to_frag: VertToFrag) -> @location(0) vec4<f32> {
    let position = vec2<u32>(vert_to_frag.texture_coord * vec2<f32>(world.size));
    let squared_distance = cursor_squared_distance(position);
    let squared_outer_radius = cursor.radius * cursor.radius;
    let squared_inner_radius = (cursor.radius - 1) * (cursor.radius - 1);
    let id = cells_output[cell_index(position)].id;
    var color = cell_colors[id];
    if squared_distance < squared_outer_radius && squared_distance >= squared_inner_radius {
        let cursor_color = cell_colors[cursor.cell_id];
        color = (cursor_color * 0.5) + ((vec3(1.0) - color) * 0.5);
    }
    return vec4(color, 1.0);
}
