#![feature(box_syntax, core)]
extern crate sdl2;
extern crate sdl2_ttf;
extern crate rand;

//todo: Fix disapering piece issues
//      Network LAN high score, More score opertunities
//      better timing, powerups? match3? More network play?

use sdl2::rect::{Rect, Point};
use sdl2::video::{Window, WindowPos, OPENGL};
use sdl2::render::{RenderDriverIndex, ACCELERATED, Renderer, RenderDrawer};
use sdl2::pixels::Color;
use sdl2::keycode::KeyCode;

use rand::{random};

const grid_width : i32 = 8;
const grid_height : i32 = 32;
const cell_width : i32 = 16;
const cell_height : i32 = 16;


fn index(x : i32, y : i32) -> i32 {
    x + y*grid_width
}

fn apply_gravity<'a>(cells : &'a mut [u8; (grid_width*grid_height) as usize]) {
    for _y in (0..grid_height-1) {
        // if there is a completly empty row, move down row above
        let y = (grid_height-1) - _y;

        let mut srow = 0u8; let mut nrow = 0u8;
        for x in (0..grid_width) {
            srow += cells[index(x, y) as usize];
            nrow += cells[index(x,y-1) as usize];
        }
        if srow == 0u8 && nrow > 0u8 {
            for x in (0..grid_width) {
                let t = index(x,y-1) as usize;
                let n = index(x,y) as usize;
                cells[n] = cells[t];
                cells[t] = 0u8;
            }
        }

    }
}

fn get_indices(t : u8, p : Point, rot : u8) -> (i32,i32,i32,i32) {
    let ofs = index(p.x, p.y);
    let (ofa, ofb, ofc, ofd) =
        match t {
            0 => (point(0,0), point(0,1), point(1,0), point(1,1)), //block
            1 => (point(0,0), point(1,0), point(2,0), point(3,0)), //stick
            2 => (point(0,0), point(1,0), point(2,0), point(2,1)), //L
            3 => (point(0,1), point(1,1), point(2,1), point(2,0)), //backwards L
            4 => (point(0,0), point(1,0), point(1,1), point(2,1)), //S
            5 => (point(0,1), point(1,0), point(1,1), point(2,0)), //backwards S
            6 => (point(0,0), point(0,1), point(0,2), point(1,1)), //T
            _ => unreachable!("Unknown piece"),
        };
    let tr =
        match rot {
            0 => (1,0,
                  0,1),
            1 => (0,-1,
                  1,0),
            2 => (-1,0,
                  0,-1),
            3 => (0,1,
                  -1,0),
            _ => unreachable!("Unknown rotation"),
        };

    let (fa,fb,fc,fd) =
        (transform(tr,ofa),transform(tr,ofb),
         transform(tr,ofc),transform(tr,ofd));

    (index(fa.x, fa.y)+ofs,
        index(fb.x, fb.y)+ofs,
        index(fc.x, fc.y)+ofs,
        index(fd.x, fd.y)+ofs)
}

fn piece_touching<'a>(t : u8, p : Point,
    cells : &'a [u8; (grid_width*grid_height) as usize], rot  :u8) -> bool {
    let (ia,ib,ic,id) = get_indices(t,p,rot);
    ia >= cells.len() as i32 ||
    ib >= cells.len() as i32 ||
    ic >= cells.len() as i32 ||
    id >= cells.len() as i32 ||
    cells[(ia) as usize] > 0u8 ||
    cells[(ib) as usize] > 0u8 ||
    cells[(ic) as usize] > 0u8 ||
    cells[(id) as usize] > 0u8
}

fn point(x : i32, y : i32) -> Point {
    Point::new(x, y)
}

fn transform(tr : (i32,i32,i32,i32), p : Point) -> Point {
    let (a,b,c,d) = tr;
    let Point{x, y} = p;
    Point::new(a*x + b*y, c*x+d*y)
}

fn place_piece<'a>(t : u8, p : Point, v : u8,
        cells : &'a mut [u8; (grid_width*grid_height) as usize], rot : u8) {
    let (ia,ib,ic,id) = get_indices(t, p, rot);
    cells[(ia) as usize] = v;
    cells[(ib) as usize] = v;
    cells[(ic) as usize] = v;
    cells[(id) as usize] = v;
}

fn draw_piece<'a, 'r>(t : u8, p : Point, rot : u8,
        drawer : &'a mut RenderDrawer<'r>, offset_x : i32, offset_y : i32) {
    let (ia,ib,ic,id) = get_indices(t,p,rot);
    drawer.fill_rect(Rect::new((ia%grid_width)*cell_width + offset_x, (ia/grid_width)*cell_height + offset_y,cell_width,cell_height));
    drawer.fill_rect(Rect::new((ib%grid_width)*cell_width + offset_x, (ib/grid_width)*cell_height + offset_y,cell_width,cell_height));
    drawer.fill_rect(Rect::new((ic%grid_width)*cell_width + offset_x, (ic/grid_width)*cell_height + offset_y,cell_width,cell_height));
    drawer.fill_rect(Rect::new((id%grid_width)*cell_width + offset_x, (id/grid_width)*cell_height + offset_y,cell_width,cell_height));
}

fn find_filled_rows<'a>(cells : &'a mut [u8; (grid_width*grid_height) as usize]) -> u32 {
    let mut filled_rows = 0u32;

    for _y in (0..grid_height-1) {
        // if there is a completly full row, count it and then clear it
        let y = (grid_height-1) - _y;
        let mut filled_cells = 0;
        for x in (0..grid_width) {
            if cells[index(x,y) as usize] > 0u8 {
                filled_cells += 1;
            }
        }
        if filled_cells >= grid_width {
            filled_rows += 1;
            for x in (0..grid_width) {
                cells[index(x,y) as usize] = 0u8;
            }
        }
    }

    filled_rows
}

enum GameState {
    Running, Paused, GameOver
}

pub fn main() {

    let sdl_context = sdl2::init(sdl2::INIT_VIDEO).unwrap();
    sdl2_ttf::init();

    let window = match Window::new(&sdl_context, "Tetris", WindowPos::PosCentered, WindowPos::PosCentered,
        grid_width*cell_width + 300, grid_height*cell_height+64, OPENGL) {
        Ok(window) => window,
        Err(err) => panic!("failed to create window: {}", err)
    };

    let mut renderer = match Renderer::from_window(window, RenderDriverIndex::Auto, ACCELERATED) {
        Ok(renderer) => renderer,
        Err(err) => panic!("failed to create renderer: {}", err)
    };

    let fnt = match sdl2_ttf::Font::from_file(
        std::path::Path::new("C:\\Windows\\Fonts\\segoeui.ttf"), 20) {
            Ok(v) => v,
            Err(e) => panic!("Error: {}", e)
        };

    let big_fnt = match sdl2_ttf::Font::from_file(
        std::path::Path::new("C:\\Windows\\Fonts\\segoeui.ttf"), 40) {
            Ok(v) => v,
            Err(e) => panic!("Error: {}", e)
        };
    let mut text_tex = renderer.create_texture_from_surface(
                &fnt.render_str_blended(
                    format!("Score: {}",
                        0).as_slice(),
                    Color::RGBA(200,200,200,255)).ok().unwrap()
            ).ok().unwrap();
    let mut gameover_tex = renderer.create_texture_from_surface(
                &big_fnt.render_str_blended(
                    "Game Over!",
                    Color::RGBA(200,20,20,255)).ok().unwrap()
            ).ok().unwrap();
    let mut pause_tex = renderer.create_texture_from_surface(
                &big_fnt.render_str_blended(
                    "~~Paused~~",
                    Color::RGBA(20,200,80,255)).ok().unwrap()
            ).ok().unwrap();

    let mut gstate = GameState::Running;

    let mut cells = [0u8; (grid_width*grid_height) as usize];

    let cell_colors = [
        Color::RGB(240, 5, 50),  //r
        Color::RGB(20, 35, 200), //g
        Color::RGB(10, 230, 30), //b
        Color::RGB(255, 220, 0), //y
        Color::RGB(0, 200, 255), //c
        Color::RGB(255, 10, 158), //m
        Color::RGB(255, 100, 10), //o
        Color::RGB(100, 50, 180), //p
    ];

    let mut cpiece_type = rand::random::<u8>()%7;
    let mut cpiece_col = rand::random::<u8>()%cell_colors.len()as u8;
    let mut cpiece_pos = Point::new(grid_width/2, 3);
    let mut cpiece_rot = rand::random::<u8>()%4;

    let mut score = 0u32;
    let mut score_dsp = 0u32;
    let mut cmprows = 0u32;
    let mut multi = 1u32;
    let mut multi_reset_timer = 0u32;

    let mut frc = 0u32;
    let mut frt = 100u32;

    let mut running = true;
    let mut event_pump = sdl_context.event_pump();
    while running {
        match gstate {
            GameState::GameOver => {
                for event in event_pump.poll_iter() {
                    use sdl2::event::Event;

                    match event {
                        Event::Quit {..} |
                            Event::KeyDown { keycode: KeyCode::Escape, .. } => {
                            running = false
                        },
                        Event::KeyDown { .. } => {
                            gstate = GameState::Running;
                            for i in (0..cells.len()) {
                                cells[i] = 0u8;
                            }
                            score = 0u32; score_dsp = 0u32; cmprows = 0u32; multi = 1u32;
                            cpiece_type = rand::random::<u8>()%7;
                            cpiece_col = rand::random::<u8>()%cell_colors.len()as u8;
                            cpiece_pos = Point::new(grid_width/2, 3);
                            cpiece_rot = rand::random::<u8>()%4;
                        }
                        _ => {},
                    }
                }
                let (txw,txh) = { let q = gameover_tex.query(); (q.width, q.height) };

                let mut drawer = renderer.drawer();

                drawer.set_draw_color(Color::RGB(0,0,0));
                drawer.clear();
                drawer.copy(&mut gameover_tex, None, Some(Rect::new(32, 200, txw, txh)));
                drawer.present();
                continue;
            },

            GameState::Paused => {
                for event in event_pump.poll_iter() {
                    use sdl2::event::Event;

                    match event {
                        Event::Quit {..} |
                            Event::KeyDown { keycode: KeyCode::Escape, .. } => {
                            running = false
                        },
                        Event::KeyDown { .. } => {
                            gstate = GameState::Running;
                        }
                        _ => {},
                    }
                }
                let (txw,txh) = { let q = pause_tex.query(); (q.width, q.height) };

                let mut drawer = renderer.drawer();

                drawer.set_draw_color(Color::RGB(0,0,0));
                drawer.clear();
                drawer.copy(&mut pause_tex, None, Some(Rect::new(64, 200, txw, txh)));
                drawer.present();
                continue;
            },
            _ => {/*just continue*/}
        }
        frc += 1u32;
        for event in event_pump.poll_iter() {
            use sdl2::event::Event;

            match event {
                Event::Quit {..} | Event::KeyDown { keycode: KeyCode::Escape, .. } => {
                    running = false
                },
                Event::KeyDown { keycode: kd, .. } => {
                        match kd {
                            KeyCode::Left => { cpiece_pos.x -= 1; },
                            KeyCode::Right => { cpiece_pos.x += 1; },
                            KeyCode::A => { cpiece_rot = (cpiece_rot+1)%4; },
                            KeyCode::D => {
                                cpiece_rot = match cpiece_rot {
                                    0 => 1,
                                    1 => 2,
                                    2 => 3,
                                    3 => 0,
                                    _ => unreachable!("SS")
                                };
                            },
                            KeyCode::Down => frt = 12u32,
                            KeyCode::P => gstate = GameState::Paused,
                            _=>{}
                        }
                },
                Event::KeyUp { keycode: KeyCode::Down, .. } => frt = 100u32,
                _ => {}
            }
        }

        let mut score_changed = false;

        if frc % frt == 0 {
            if piece_touching(cpiece_type, Point::new(cpiece_pos.x, cpiece_pos.y+1), &cells, cpiece_rot) {
                place_piece(cpiece_type, cpiece_pos, cpiece_col, &mut cells, cpiece_rot);
                if cpiece_pos.y > grid_height-3 {
                    score_changed = true;
                    score += 50;
                }
                cpiece_type = rand::random::<u8>()%7;
                cpiece_col = rand::random::<u8>()%cell_colors.len()as u8;
                cpiece_pos = Point::new(grid_width/2, 3);
                cpiece_rot = rand::random::<u8>()%4;
                if piece_touching(cpiece_type, cpiece_pos, &cells, cpiece_rot) {
                    gstate = GameState::GameOver;
                }
            } else {
                cpiece_pos.y += 1;
            }
        }

        if multi > 1 { multi_reset_timer += 1u32; }
        let fr = find_filled_rows(&mut cells);
        cmprows += fr; score += fr*100*multi;
        if fr >= 1 && fr < 4 { multi+=1u32; }
        else if fr >= 4 { score += 10000; }
        if multi > 1 && multi_reset_timer >= 10000u32 { multi = 1u32; score_changed = true; multi_reset_timer = 0; }
        apply_gravity(&mut cells);

        if score != score_dsp {
            score_dsp += if score > score_dsp { 1 } else { -1 };
        }

        if fr > 0  || score_changed || score != score_dsp {
            text_tex = renderer.create_texture_from_surface(
                    &fnt.render_str_blended(
                        format!("Score: {} [x{} bonus]",
                            score_dsp, multi).as_slice(),
                        Color::RGBA(200,200,200,255)).ok().unwrap()
                ).ok().unwrap();
        }
        let (txw,txh) = { let q = text_tex.query(); (q.width, q.height) };

        let mut drawer = renderer.drawer();

        drawer.set_draw_color(Color::RGB(0,0,0));
        drawer.clear();

        drawer.set_draw_color(Color::RGB(100,100,120));
        let (offset_x, offset_y) = (32,32);
        for x in (0..grid_width+1) {
            drawer.draw_line(Point::new(x*cell_width + offset_x, offset_y),
                 Point::new(x*cell_width + offset_x, offset_y+grid_height*cell_height));
        }
        for y in (0..grid_height+1) {
            drawer.draw_line(Point::new(offset_x, offset_y+y*cell_height),
                Point::new(offset_x+grid_width*cell_width, offset_y+y*cell_height));
        }
        for i in (0..grid_height*grid_width) {
            let x = i % grid_width;
            let y = i / grid_width;
            let v = cells[i as usize];
            if v > 0 {
                drawer.set_draw_color(cell_colors[v as usize]);
                drawer.fill_rect(Rect::new(offset_x+x*cell_width,
                    offset_y+y*cell_height, cell_width, cell_height));
            }
        }

        drawer.set_draw_color(cell_colors[cpiece_col as usize]);
        draw_piece(cpiece_type, cpiece_pos,
            cpiece_rot, &mut drawer, offset_x, offset_y);

        drawer.copy(&mut text_tex, None, Some(Rect::new(cell_width*grid_width + 48, 32, txw, txh)));

        drawer.present();
    }
}
