use std::{borrow::Cow, rc::Rc};

use background_shader::BackgroundShader;
use image::{io::Reader as ImageReader, ImageBuffer, Luma, Rgba};

use clap::{Parser, Subcommand};
use glium::{
    framebuffer::{MultiOutputFrameBuffer, SimpleFrameBuffer},
    glutin::surface::WindowSurface,
    implement_vertex,
    texture::{DepthTexture2d, RawImage2d},
    uniform, Display, DrawParameters, Program, Surface, Texture2d, VertexBuffer,
};
use nalgebra::{Matrix4, Point3, Vector3, Vector4};
use winit::{
    event::{Event, WindowEvent},
    window::Window,
};

mod background_shader;

#[derive(Copy, Clone, Debug)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 4],
}
implement_vertex!(Vertex, position, color);

struct ViewParams {
    eye: Point3<f32>,
    look_at: Point3<f32>,
    roll: f32,
    pitch: f32,
    yaw: f32,
    camera: Matrix4<f32>,
    projection: Matrix4<f32>,
}

impl ViewParams {
    pub fn new(eye: Point3<f32>, look_at: Point3<f32>, projection: Matrix4<f32>) -> Self {
        ViewParams {
            eye,
            look_at,
            roll: 0.0,
            pitch: 0.0,
            yaw: 0.0,
            camera: Matrix4::look_at_rh(&eye, &look_at, &Vector3::new(0.0, 1.0, 0.0))
                * Matrix4::from_euler_angles(0.0, 0.0, 0.0),
            projection,
        }
    }

    fn update_camera(&mut self) {
        self.camera = Matrix4::look_at_rh(&self.eye, &self.look_at, &Vector3::new(0.0, 1.0, 0.0))
            * Matrix4::from_euler_angles(self.roll, self.pitch, self.yaw);
    }

    pub fn set_eye(&mut self, eye: Point3<f32>) {
        self.eye = eye;
        self.update_camera();
    }
    pub fn set_look_at(&mut self, look_at: Point3<f32>) {
        self.look_at = look_at;
        self.update_camera();
    }
    pub fn set_roll(&mut self, roll: f32) {
        self.roll = roll;
        self.update_camera();
    }
    pub fn set_pitch(&mut self, pitch: f32) {
        self.pitch = pitch;
        self.update_camera();
    }

    pub fn set_yaw(&mut self, yaw: f32) {
        self.yaw = yaw;
        self.update_camera();
    }
}
struct Renderer {
    display: Rc<Display<WindowSurface>>,
    program: Program,
    vertex_buffer: VertexBuffer<Vertex>,
    target_texture: Texture2d,
    target_depth: Texture2d,
    view_params: ViewParams,
    background_shader: Option<BackgroundShader>,
    raster: bool,
}

impl Renderer {
    pub fn new(
        display: Display<WindowSurface>,
        image: ImageBuffer<Rgba<u8>, Vec<u8>>,
        depth: ImageBuffer<Luma<u8>, Vec<u8>>,
        background_filling: bool,
        raster: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        assert_eq!(image.dimensions(), depth.dimensions());
        let dims = image.dimensions();
        let program = Program::from_source(
            &display,
            include_str!("vertex.glsl"),
            include_str!("fragment.glsl"),
            None,
        )?;
        let mut vertices = Vec::with_capacity((dims.0 * dims.1) as usize);
        let min_depth = depth.rows().flatten().map(|luma| luma.0[0]).min().unwrap();
        let max_depth =
            (depth.rows().flatten().map(|luma| luma.0[0]).max().unwrap() - min_depth) as f32;
        // Generate vertices for each pixel. OpenGL coordinates have a minimum of -1 and maximum of 1
        for (y, (r1, r2)) in image.rows().zip(depth.rows()).enumerate() {
            for (x, (c1, c2)) in r1.zip(r2).enumerate() {
                vertices.push(Vertex {
                    position: [
                        (x as f32 / dims.0 as f32) * 2.0 - 1.0,
                        // Top of the screen is +1 in OpenGL
                        (y as f32 / dims.1 as f32) * -2.0 + 1.0,
                        ((c2.0[0] - min_depth) as f32 / (max_depth - min_depth as f32)) * -2.0
                            + 0.9,
                    ],
                    color: [
                        c1.0[0] as f32 / 255.0,
                        c1.0[1] as f32 / 255.0,
                        c1.0[2] as f32 / 255.0,
                        0.0,
                    ],
                });
            }
        }
        println!(
            "Min depth: {:?}",
            vertices
                .iter()
                .map(|v| float_ord::FloatOrd(v.position[2]))
                .min()
                .unwrap()
        );
        println!(
            "Max depth: {:?}",
            vertices
                .iter()
                .map(|v| float_ord::FloatOrd(v.position[2]))
                .max()
                .unwrap()
        );
        let vertex_buffer = VertexBuffer::new(&display, &vertices)?;

        let eye = Point3::new(0.0f32, 0.0, 1.0);
        let look_at = Point3::new(0.0, 0.0, -0.1);

        // TODO: figure out projection. This is just a placeholder
        let view_params = ViewParams::new(
            eye,
            look_at,
            Matrix4::new_orthographic(-1.0f32, 1.0, -1.0, 1.0, 0.0, 3.0),
        );

        // println!("Min depth camera: {:?}", vertices.iter().map(|v| view_params.camera * Vector4::new(v.position[0], v.position[1], v.position[2], 1.0)).
        //                                                 map(|v| float_ord::FloatOrd(v[2])).min().unwrap());
        // println!("Max depth camera: {:?}", vertices.iter().map(|v| view_params.camera * Vector4::new(v.position[0], v.position[1], v.position[2], 1.0)).
        //                                                 map(|v| float_ord::FloatOrd(v[2])).max().unwrap());
        // println!("Min depth projection: {:?}", vertices.iter().map(|v| view_params.projection * view_params.camera * Vector4::new(v.position[0], v.position[1], v.position[2], 1.0)).
        //                                                 map(|v| float_ord::FloatOrd(v[2])).min().unwrap());
        // println!("Max depth projection: {:?}", vertices.iter().map(|v| view_params.projection * view_params.camera * Vector4::new(v.position[0], v.position[1], v.position[2], 1.0)).
        //                                                 map(|v| float_ord::FloatOrd(v[2])).max().unwrap());

        let display = Rc::new(display);
        let target_texture = Texture2d::empty_with_format(
            &*display,
            glium::texture::UncompressedFloatFormat::U8U8U8U8,
            glium::texture::MipmapsOption::NoMipmap,
            dims.0,
            dims.1,
        )?;
        let target_depth = Texture2d::empty_with_format(
            &*display,
            glium::texture::UncompressedFloatFormat::F32,
            glium::texture::MipmapsOption::NoMipmap,
            dims.0,
            dims.1,
        )?;

        let raw_image = RawImage2d::from_raw_rgba_reversed(&image.to_vec(), dims);
        target_texture.write(
            glium::Rect {
                left: 0,
                bottom: 0,
                width: dims.0,
                height: dims.1,
            },
            raw_image,
        );
        let raw_depth = RawImage2d {
            data: Cow::Owned(image::imageops::flip_vertical(&depth).to_vec()),
            format: glium::texture::ClientFormat::U8,
            width: dims.0,
            height: dims.1,
        };
        target_depth.write(
            glium::Rect {
                left: 0,
                bottom: 0,
                width: dims.0,
                height: dims.1,
            },
            raw_depth,
        );

        let background_shader = if background_filling {
            Some(BackgroundShader::new(display.clone(), dims)?)
        } else {
            None
        };

        Ok(Self {
            display,
            program,
            vertex_buffer,
            target_texture,
            target_depth,
            view_params,
            background_shader,
            raster,
        })
    }

    fn render_to<S: glium::Surface>(&self, target: &mut S) {
        target.clear_depth(1.0);
        target.clear_color(0.0, 0.0, 0.0, 1.0);

        let uniforms = uniform! {
            projectionview: *(self.view_params.projection * self.view_params.camera).as_ref(),
        };
        let mut draw_options = DrawParameters::default();
        draw_options.depth.test = glium::draw_parameters::DepthTest::IfLessOrEqual;
        draw_options.depth.write = true;
        draw_options.point_size = Some(1.0);
        target
            .draw(
                &self.vertex_buffer,
                &glium::index::NoIndices(glium::index::PrimitiveType::Points),
                &self.program,
                &uniforms,
                &draw_options,
            )
            .unwrap();
    }

    // TODO: remove toggle
    fn render(&mut self, toggle: bool) -> Result<(), Box<dyn std::error::Error>> {
        let target = self.display.draw();
        if self.raster {
            let dims = target.get_dimensions();
            // TODO: don't create new textures on every render iteration
            let depth_buffer = DepthTexture2d::empty(&*self.display, dims.0, dims.1)?;

            let outputs = [
                ("color_out", &self.target_texture),
                ("depth_out", &self.target_depth),
            ];
            let mut framebuffer = MultiOutputFrameBuffer::with_depth_buffer(
                &*self.display,
                outputs.iter().cloned(),
                &depth_buffer,
            )?;
            self.render_to(&mut framebuffer);
        }

        if let Some(background_shader) = &mut self.background_shader {
            if toggle {
                background_shader.run(&self.target_texture, &self.target_depth)?;
                let (color, _depth) = background_shader.front_buffer();
                color.sync_shader_writes_for_surface();
                color
                    .as_surface()
                    .fill(&target, glium::uniforms::MagnifySamplerFilter::Nearest);
            }
        }
        if !toggle {
            // Multi-output framebuffers don't support fill()
            let simple_buffer = SimpleFrameBuffer::new(&*self.display, &self.target_texture)?;
            simple_buffer.fill(&target, glium::uniforms::MagnifySamplerFilter::Nearest);
        }
        target.finish()?;

        Ok(())
    }

    fn save_screenshot(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let image: RawImage2d<'_, u8> = self.display.read_front_buffer()?;
        let image_buffer =
            ImageBuffer::from_raw(image.width, image.height, image.data.into_owned()).unwrap();
        let image = image::DynamicImage::ImageRgba8(image_buffer).flipv();
        image.save(name)?;

        Ok(())
    }
    fn save_depth(&self, name: &str) {
        let depth_texture = if let Some(background_shader) = self.background_shader.as_ref() {
            &background_shader.front_buffer().1
        } else {
            eprintln!("WARNING: Background shading disabled. Reading depth map from target depth");
            &self.target_depth
        };
        unsafe {
            let output: RawImage2d<'static, f32> =
                depth_texture.unchecked_read::<RawImage2d<'static, f32>, f32>();
            let image_buffer: ImageBuffer<image::Luma<u8>, Vec<u8>> = ImageBuffer::from_vec(output.width, output.height,
            output.data.iter().copied().map(|f| (f * 255.0) as u8).collect::<Vec<u8>>()).unwrap();

            let image = image::DynamicImage::ImageLuma8(image_buffer).flipv();

            image.save(name).unwrap();
        }
    }
}

fn open_display(
    event_loop: &winit::event_loop::EventLoop<()>,
    width: u32,
    height: u32,
) -> (Window, Display<WindowSurface>) {
    // Boilerplate code ripped from glium git
    use glutin::display::GetGlDisplay;
    use glutin::prelude::*;
    use raw_window_handle::HasRawWindowHandle;

    // First we start by opening a new Window
    let builder = winit::window::WindowBuilder::new()
        .with_inner_size(winit::dpi::PhysicalSize::new(width, height));
    let display_builder = glutin_winit::DisplayBuilder::new().with_window_builder(Some(builder));
    let config_template_builder = glutin::config::ConfigTemplateBuilder::new();

    let (window, gl_config) = display_builder
        .build(&event_loop, config_template_builder, |mut configs| {
            // Just use the first configuration since we don't have any special preferences here
            configs.next().unwrap()
        })
        .unwrap();
    let window = window.unwrap();

    // Now we get the window size to use as the initial size of the Surface
    let (width, height): (u32, u32) = window.inner_size().into();
    let attrs = glutin::surface::SurfaceAttributesBuilder::<glutin::surface::WindowSurface>::new()
        .build(
            window.raw_window_handle(),
            std::num::NonZeroU32::new(width).unwrap(),
            std::num::NonZeroU32::new(height).unwrap(),
        );

    // Finally we can create a Surface, use it to make a PossiblyCurrentContext and create the glium Display
    let surface = unsafe {
        gl_config
            .display()
            .create_window_surface(&gl_config, &attrs)
            .unwrap()
    };
    let context_attributes = glutin::context::ContextAttributesBuilder::new()
        .with_context_api(glutin::context::ContextApi::OpenGl(Some(
            glutin::context::Version { major: 4, minor: 6 },
        )))
        .build(Some(window.raw_window_handle()));
    let current_context = Some(unsafe {
        gl_config
            .display()
            .create_context(&gl_config, &context_attributes)
            .expect("failed to create context")
    })
    .unwrap()
    .make_current(&surface)
    .unwrap();
    let display = Display::from_context_surface(current_context, surface).unwrap();
    (window, display)
}

#[derive(Parser)]
struct Args {
    image_path: String,
    depth_path: String,
    before_path: Option<String>,
    mask_path: Option<String>,
}

fn get_image(
    args: &Args,
) -> Result<
    (
        ImageBuffer<Rgba<u8>, Vec<u8>>,
        ImageBuffer<Luma<u8>, Vec<u8>>,
    ),
    Box<dyn std::error::Error>,
> {
    let img = ImageReader::open(&args.image_path)?.decode()?.to_rgba8();
    let mut depth = ImageReader::open(&args.depth_path)?.decode()?.to_luma8();
    //depth.save("/tmp/foo.png")?;
    assert_eq!(img.dimensions(), depth.dimensions());

    let mut test_image: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        ImageBuffer::new(img.dimensions().0, img.dimensions().1);

    if let Some(before_path) = &args.before_path {
        if let Some(mask_path) = &args.mask_path {
            let before = ImageReader::open(&before_path)?.decode()?.to_rgba8();
            let mask = ImageReader::open(&mask_path)?.decode()?.to_luma8();
            for (i, (((maskrow, beforerow), afterrow), depthrow)) in mask
                .rows()
                .zip(before.rows())
                .zip(img.rows())
                .zip(depth.rows_mut())
                .enumerate()
            {
                for (j, (((mask, before), after), depth)) in maskrow
                    .zip(beforerow)
                    .zip(afterrow)
                    .zip(depthrow)
                    .enumerate()
                {
                    let beforev =
                        Vector3::new(before.0[0] as f32, before.0[1] as f32, before.0[2] as f32);
                    let afterv =
                        Vector3::new(after.0[0] as f32, after.0[1] as f32, after.0[2] as f32);
                    if (afterv - beforev).abs().magnitude() < 30.0 && mask.0[0] > 200 {
                        if mask.0[0] > 200 {
                            depth.0[0] = 0;
                            test_image.get_pixel_mut(j as u32, i as u32).0[0] = 255;
                        } else {
                            // Max depth to avoid background shading, probably a better way to do this by adding a mask input to the compute shader
                            depth.0[0] = 255;
                            test_image.get_pixel_mut(j as u32, i as u32).0[1] = 255;
                        }
                    }
                }
            }
        }
    }
    //test_image.save("/tmp/foo2.png")?;
    Ok((img, depth))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let (image, depth) = get_image(&args).unwrap();
    let dims = image.dimensions();

    let events_loop = winit::event_loop::EventLoopBuilder::new().build();

    let (_window, display) = open_display(&events_loop, dims.0, dims.1);

    let mut renderer = Renderer::new(display, image, depth, true, args.mask_path.is_none())?;

    let mut changed = true;
    let mut img_count = 0;
    let mut toggle = true;
    events_loop.run(move |e, _, ctrl| match e {
        Event::WindowEvent {
            event: WindowEvent::ReceivedCharacter('a'),
            ..
        } => {
            renderer
                .view_params
                .set_pitch(renderer.view_params.pitch + 0.01);
        }
        Event::WindowEvent {
            event: WindowEvent::ReceivedCharacter('d'),
            ..
        } => {
            renderer
                .view_params
                .set_pitch(renderer.view_params.pitch - 0.01);
        }
        Event::WindowEvent {
            event: WindowEvent::ReceivedCharacter('q'),
            ..
        } => {
            renderer
                .view_params
                .set_yaw(renderer.view_params.yaw + 0.01);
        }
        Event::WindowEvent {
            event: WindowEvent::ReceivedCharacter('e'),
            ..
        } => {
            renderer
                .view_params
                .set_yaw(renderer.view_params.yaw - 0.01);
        }
        Event::WindowEvent {
            event: WindowEvent::ReceivedCharacter('w'),
            ..
        } => {
            renderer
                .view_params
                .set_roll(renderer.view_params.roll + 0.01);
        }
        Event::WindowEvent {
            event: WindowEvent::ReceivedCharacter('s'),
            ..
        } => {
            renderer
                .view_params
                .set_roll(renderer.view_params.roll - 0.01);
        }
        Event::WindowEvent {
            event: WindowEvent::ReceivedCharacter('f'),
            ..
        } => {
            // set changed to true. this tells the renderer it should take a screenshot on the next frame
            changed = true;
        }
        Event::WindowEvent {
            event: WindowEvent::ReceivedCharacter('t'),
            ..
        } => {
            // enable background filling
            toggle = !toggle;
        }
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => ctrl.set_exit_with_code(0),

        Event::MainEventsCleared => {
            renderer.render(toggle).unwrap();
            if changed {
                renderer
                    .save_screenshot(&format!("screenshot-{}.png", img_count))
                    .unwrap();
                renderer
                    .save_depth(&format!("screenshot-depth-{}.png", img_count));
                img_count += 1;
                changed = false;
            }
        }
        _ => {}
    });
}
