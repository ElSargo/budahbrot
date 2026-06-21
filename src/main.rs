mod renderer;

use iced::time;
use iced::widget::canvas;
use iced::widget::{
    button, column, container, pick_list, progress_bar, row, scrollable, slider, text, text_input,
    Canvas,
};
use iced::{
    font, mouse, Alignment, Background, Border, Color, Element, Font, Length, Point, Rectangle,
    Renderer, Size, Subscription, Task, Theme, Vector,
};
use renderer::{RenderJob, RenderSettings, RenderSnapshot, ToneMapping, DEFAULT_SAMPLES_PER_PIXEL};
use std::cell::RefCell;
use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

const CHANNEL_NAMES: [&str; 3] = ["Red", "Green", "Blue"];
const RESOLUTION_PRESETS: [usize; 7] = [256, 450, 720, 1080, 1440, 2160, 4096];
const PREVIEW_INTERVAL_PRESETS: [u64; 5] = [50, 100, 250, 500, 1000];
const SAMPLE_SLIDER_MAX: u16 = 2_500;
const DEFAULT_GAMMA: f32 = 0.6;
const OUTLINE_MAX_RESOLUTION: u32 = 1200;
const ICON_FONT_BYTES: &[u8] = include_bytes!("../assets/fa-solid-900.ttf");
const ICON_FONT: Font = Font {
    family: font::Family::Name("Font Awesome 6 Free"),
    weight: font::Weight::Black,
    stretch: font::Stretch::Normal,
    style: font::Style::Normal,
};
const ICON_PLAY: &str = "\u{f04b}";
const ICON_PAUSE: &str = "\u{f04c}";
const ICON_STOP: &str = "\u{f04d}";
const ICON_SAVE: &str = "\u{f019}";
const ICON_PLUS: &str = "\u{2b}";

#[derive(Debug, Clone, Copy)]
struct AppStyle {
    app_background: Color,
    preview_background: Color,
    border: Color,
    text: Color,
    muted_text: Color,
    primary: Color,
    success: Color,
    danger: Color,
    control_width: f32,
    outer_padding: u16,
    panel_padding: u16,
    preview_padding: u16,
    footer_padding: u16,
    gap: u16,
    radius: f32,
    progress_height: f32,
}

impl AppStyle {
    fn spectral_lab() -> Self {
        Self {
            app_background: rgb(13, 9, 18),
            preview_background: rgb(5, 4, 9),
            border: rgb(91, 70, 119),
            text: rgb(246, 239, 255),
            muted_text: rgb(180, 166, 199),
            primary: rgb(247, 92, 160),
            success: rgb(87, 211, 164),
            danger: rgb(255, 105, 97),
            control_width: 318.0,
            outer_padding: 10,
            panel_padding: 14,
            preview_padding: 10,
            footer_padding: 10,
            gap: 10,
            radius: 4.0,
            progress_height: 10.0,
        }
    }
}

fn app_style() -> AppStyle {
    AppStyle::spectral_lab()
}

fn app_theme() -> Theme {
    let style = app_style();
    Theme::custom(
        String::from("Budahbrot"),
        iced::theme::Palette {
            background: style.app_background,
            text: style.text,
            primary: style.primary,
            success: style.success,
            danger: style.danger,
        },
    )
}

fn rgb(red: u8, green: u8, blue: u8) -> Color {
    Color::from_rgb8(red, green, blue)
}

fn app_container_style(style: AppStyle) -> iced::widget::container::Style {
    iced::widget::container::Style {
        text_color: Some(style.text),
        background: Some(Background::Color(style.app_background)),
        ..iced::widget::container::Style::default()
    }
}

fn panel_container_style(style: AppStyle) -> iced::widget::container::Style {
    iced::widget::container::Style {
        text_color: Some(style.text),
        ..iced::widget::container::Style::default()
    }
}

fn preview_container_style(style: AppStyle) -> iced::widget::container::Style {
    iced::widget::container::Style {
        text_color: Some(style.text),
        ..iced::widget::container::Style::default()
    }
}

fn footer_container_style(style: AppStyle) -> iced::widget::container::Style {
    iced::widget::container::Style {
        text_color: Some(style.text),
        ..iced::widget::container::Style::default()
    }
}

fn progress_style(style: AppStyle) -> iced::widget::progress_bar::Style {
    iced::widget::progress_bar::Style {
        background: Background::Color(style.border),
        bar: Background::Color(style.primary),
        border: Border {
            radius: (style.radius / 2.0).into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
    }
}

#[derive(Debug, Clone, Copy)]
enum ActionRole {
    Primary,
    Neutral,
    Danger,
    Save,
}

fn action_button_style(
    style: AppStyle,
    role: ActionRole,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let accent = match role {
        ActionRole::Primary => style.success,
        ActionRole::Neutral => style.primary,
        ActionRole::Danger => style.danger,
        ActionRole::Save => style.primary,
    };

    let alpha = match status {
        iced::widget::button::Status::Hovered => 0.38,
        iced::widget::button::Status::Pressed => 0.48,
        iced::widget::button::Status::Disabled => 0.12,
        iced::widget::button::Status::Active => 0.24,
    };

    let text_color = if status == iced::widget::button::Status::Disabled {
        rgba(style.muted_text, 0.48)
    } else {
        style.text
    };

    iced::widget::button::Style {
        background: Some(Background::Color(rgba(accent, alpha))),
        text_color,
        border: Border {
            radius: style.radius.into(),
            width: 1.0,
            color: if status == iced::widget::button::Status::Disabled {
                rgba(style.border, 0.42)
            } else {
                rgba(accent, 0.8)
            },
        },
        ..iced::widget::button::Style::default()
    }
}

fn rgba(color: Color, alpha: f32) -> Color {
    Color { a: alpha, ..color }
}

fn action_button_content(
    icon: &'static str,
    label: Option<&'static str>,
) -> Element<'static, Message> {
    let icon_text = text(icon).font(ICON_FONT).size(15).center();

    match label {
        Some(label) => container(
            row![icon_text.width(Length::Fixed(18.0)), text(label).size(14),]
                .spacing(8)
                .align_y(Alignment::Center),
        )
        .center_x(Length::Fill)
        .into(),
        None => container(icon_text.width(Length::Fill))
            .center_x(Length::Fill)
            .into(),
    }
}

fn main() -> iced::Result {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let start_on_run = args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--run-page" | "--run"));

    iced::application(
        BudahbrotApp::title,
        BudahbrotApp::update,
        BudahbrotApp::view,
    )
    .font(ICON_FONT_BYTES)
    .subscription(BudahbrotApp::subscription)
    .theme(BudahbrotApp::theme)
    .run_with(move || {
        let mut app = BudahbrotApp::new();
        if start_on_run {
            app.active_tab = ControlsTab::Run;
            app.start_render();
        }

        (app, Task::none())
    })
}

struct BudahbrotApp {
    active_tab: ControlsTab,
    fields: SettingsFields,
    pending_bounds: Option<PlaneBounds>,
    output_file_name: String,
    last_save_dir: Option<PathBuf>,
    gamma: f32,
    job: Option<RenderJob>,
    preview: PreviewImage,
    preview_plane: CPlane,
    status: String,
}

impl Default for BudahbrotApp {
    fn default() -> Self {
        Self::new()
    }
}

impl BudahbrotApp {
    fn new() -> Self {
        let settings = RenderSettings::default();

        Self {
            active_tab: ControlsTab::Generate,
            fields: SettingsFields::from_settings(&settings),
            pending_bounds: None,
            output_file_name: String::from("result.png"),
            last_save_dir: None,
            gamma: DEFAULT_GAMMA,
            job: None,
            preview: PreviewImage::blank(settings.resolution),
            preview_plane: CPlane::from_settings(&settings),
            status: String::from("Configure a render and switch to Run."),
        }
    }

    fn title(&self) -> String {
        String::from("Budahbrot renderer")
    }

    fn theme(&self) -> Theme {
        app_theme()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControlsTab {
    Generate,
    Run,
}

#[derive(Debug, Clone)]
enum Message {
    TabSelected(ControlsTab),
    WorkersSliderChanged(u16),
    ResolutionPresetSelected(usize),
    SamplesSliderChanged(u16),
    PowerChanged(f32),
    GammaChanged(f32),
    PreviewIntervalPresetSelected(u64),
    PreviewInteractionChanged,
    BoundsSelected(PlaneBounds),
    BoundsReset,
    RedIterationsChanged(String),
    GreenIterationsChanged(String),
    BlueIterationsChanged(String),
    RedWeightChanged(String),
    GreenWeightChanged(String),
    BlueWeightChanged(String),
    PlayPressed,
    PausePressed,
    StopPressed,
    ExtendSamplesPressed,
    SavePressed,
    Tick,
}

impl BudahbrotApp {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(tab) => {
                let was_run = self.active_tab == ControlsTab::Run;
                self.active_tab = tab;

                if tab == ControlsTab::Run && !was_run && !self.has_active_render() {
                    self.start_render();
                }
            }
            Message::WorkersSliderChanged(value) => self.fields.workers = value.to_string(),
            Message::ResolutionPresetSelected(value) => {
                self.fields.resolution = value.to_string();
                self.fields.samples_per_pixel = recommended_samples_per_pixel(value).to_string();
            }
            Message::SamplesSliderChanged(value) => {
                self.fields.samples_per_pixel = value.to_string()
            }
            Message::PowerChanged(value) => self.fields.power = value,
            Message::GammaChanged(value) => {
                self.gamma = value;
                self.refresh_preview();
            }
            Message::PreviewIntervalPresetSelected(value) => {
                self.fields.preview_interval_ms = value.to_string();
            }
            Message::PreviewInteractionChanged => {}
            Message::BoundsSelected(bounds) => {
                self.pending_bounds = Some(bounds);
                self.status = String::from("Bounds selected for the next render.");
            }
            Message::BoundsReset => {
                let defaults = RenderSettings::default();
                self.pending_bounds = None;
                self.set_bounds(PlaneBounds {
                    real_min: defaults.real_min,
                    real_max: defaults.real_max,
                    imaginary_min: defaults.imaginary_min,
                    imaginary_max: defaults.imaginary_max,
                });
                self.status = String::from("Bounds reset for the next render.");
            }
            Message::RedIterationsChanged(value) => self.fields.iterations[0] = value,
            Message::GreenIterationsChanged(value) => self.fields.iterations[1] = value,
            Message::BlueIterationsChanged(value) => self.fields.iterations[2] = value,
            Message::RedWeightChanged(value) => self.fields.weights[0] = value,
            Message::GreenWeightChanged(value) => self.fields.weights[1] = value,
            Message::BlueWeightChanged(value) => self.fields.weights[2] = value,
            Message::PlayPressed => self.play_render(),
            Message::PausePressed => self.pause_render(),
            Message::StopPressed => self.stop_render(),
            Message::ExtendSamplesPressed => self.extend_samples(),
            Message::SavePressed => self.save_current_image(),
            Message::Tick => self.refresh_preview(),
        }

        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        let is_rendering = self
            .job
            .as_ref()
            .map(|job| !job.is_finished())
            .unwrap_or(false);

        if is_rendering {
            let interval = self
                .fields
                .preview_interval_ms
                .parse::<u64>()
                .unwrap_or(250);
            time::every(Duration::from_millis(interval.clamp(16, 5_000))).map(|_| Message::Tick)
        } else {
            Subscription::none()
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let style = app_style();
        let tabs = row![
            tab_button("Gen", ControlsTab::Generate, self.active_tab),
            tab_button("Run", ControlsTab::Run, self.active_tab)
        ]
        .spacing(8);

        let tab_controls = match self.active_tab {
            ControlsTab::Generate => self.generation_controls(),
            ControlsTab::Run => self.run_controls(),
        };

        let controls = column![tabs, tab_controls].spacing(12);

        let preview_handle = iced::widget::image::Handle::from_rgba(
            self.preview.width,
            self.preview.height,
            self.preview.rgba.clone(),
        );
        let preview = Canvas::new(PreviewSurface {
            handle: preview_handle,
            width: self.preview.width,
            height: self.preview.height,
            plane: self.preview_plane(),
            pending_bounds: self.pending_bounds,
            show_outline: self.active_tab == ControlsTab::Generate && self.job.is_none(),
            background: style.preview_background,
            border: style.border,
        })
        .width(Length::Fill)
        .height(Length::Fill);

        let controls_panel = container(scrollable(controls))
            .width(Length::Fixed(style.control_width))
            .height(Length::Fill)
            .padding(style.panel_padding)
            .style(move |_theme| panel_container_style(style));

        let preview_panel = container(
            column![
                container(preview)
                    .height(Length::Fill)
                    .padding(style.preview_padding)
                    .style(move |_theme| preview_container_style(style)),
                self.preview_footer()
            ]
            .spacing(style.gap),
        )
        .width(Length::Fill)
        .height(Length::Fill);

        let layout = row![controls_panel, preview_panel].spacing(style.gap);

        container(layout)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(style.outer_padding)
            .style(move |_theme| app_container_style(style))
            .into()
    }

    fn gamma_control(&self) -> Element<'_, Message> {
        row![
            text(format!("gamma = {:.2}", self.gamma)).width(Length::Fixed(104.0)),
            slider(0.2..=4.0, self.gamma, Message::GammaChanged)
                .step(0.05)
                .default(DEFAULT_GAMMA),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    }

    fn preview_footer(&self) -> Element<'_, Message> {
        let style = app_style();
        let extend_button = if self.job.is_some() {
            button(action_button_content(ICON_PLUS, None))
                .width(Length::Fixed(38.0))
                .height(Length::Fixed(30.0))
                .padding(6)
                .style(move |_theme, status| {
                    action_button_style(style, ActionRole::Neutral, status)
                })
                .on_press(Message::ExtendSamplesPressed)
        } else {
            button(action_button_content(ICON_PLUS, None))
                .width(Length::Fixed(38.0))
                .height(Length::Fixed(30.0))
                .padding(6)
                .style(move |_theme, status| {
                    action_button_style(style, ActionRole::Neutral, status)
                })
        };

        let content = column![
            row![
                text(&self.status).width(Length::Fill),
                text(self.preview.progress_label())
            ]
            .spacing(12)
            .align_y(Alignment::Center),
            row![
                progress_bar(0.0..=1.0, self.preview.progress_fraction())
                    .width(Length::Fill)
                    .height(Length::Fixed(style.progress_height))
                    .style(move |_theme| progress_style(style)),
                extend_button
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(8);

        container(content)
            .padding(style.footer_padding)
            .style(move |_theme| footer_container_style(style))
            .into()
    }

    fn generation_controls(&self) -> Element<'_, Message> {
        let worker_value = parse_u16_or(&self.fields.workers, 20).clamp(1, 256);
        let sample_value = parse_u16_or(
            &self.fields.samples_per_pixel,
            DEFAULT_SAMPLES_PER_PIXEL as u16,
        )
        .clamp(1, SAMPLE_SLIDER_MAX);
        let selected_resolution = self
            .fields
            .resolution
            .parse::<usize>()
            .ok()
            .filter(|value| RESOLUTION_PRESETS.contains(value));
        let selected_preview_interval = self
            .fields
            .preview_interval_ms
            .parse::<u64>()
            .ok()
            .filter(|value| PREVIEW_INTERVAL_PRESETS.contains(value));

        column![
            text("Equation").size(16),
            row![
                text(format!("k = {:.2}", self.fields.power)).width(Length::Fixed(72.0)),
                slider(1.0..=12.0, self.fields.power, Message::PowerChanged)
                    .step(0.05)
                    .default(2.0),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            text("Bounds").size(16),
            text(self.bounds_label()).size(12),
            button(text("Reset bounds")).on_press(Message::BoundsReset),
            text("Color").size(16),
            channel_fields(
                CHANNEL_NAMES[0],
                &self.fields.iterations[0],
                Message::RedIterationsChanged,
                &self.fields.weights[0],
                Message::RedWeightChanged,
            ),
            channel_fields(
                CHANNEL_NAMES[1],
                &self.fields.iterations[1],
                Message::GreenIterationsChanged,
                &self.fields.weights[1],
                Message::GreenWeightChanged,
            ),
            channel_fields(
                CHANNEL_NAMES[2],
                &self.fields.iterations[2],
                Message::BlueIterationsChanged,
                &self.fields.weights[2],
                Message::BlueWeightChanged,
            ),
            text("Sampling").size(16),
            row![
                text("Resolution").width(Length::Fixed(90.0)),
                pick_list(
                    RESOLUTION_PRESETS.as_slice(),
                    selected_resolution,
                    Message::ResolutionPresetSelected
                )
                .width(Length::Fill),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            row![
                text(format!("samples = {sample_value}")).width(Length::Fixed(112.0)),
                slider(
                    1_u16..=SAMPLE_SLIDER_MAX,
                    sample_value,
                    Message::SamplesSliderChanged
                )
                .step(1_u16),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            text("Performance").size(16),
            row![
                text(format!("workers = {worker_value}")).width(Length::Fixed(112.0)),
                slider(1_u16..=256_u16, worker_value, Message::WorkersSliderChanged).step(1_u16),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            row![
                text("Preview ms").width(Length::Fixed(90.0)),
                pick_list(
                    PREVIEW_INTERVAL_PRESETS.as_slice(),
                    selected_preview_interval,
                    Message::PreviewIntervalPresetSelected
                )
                .width(Length::Fill),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(10)
        .into()
    }

    fn c_plane(&self) -> CPlane {
        let defaults = RenderSettings::default();
        let mut plane = CPlane {
            real_min: parse_f64_or(&self.fields.real_min, defaults.real_min),
            real_max: parse_f64_or(&self.fields.real_max, defaults.real_max),
            imaginary_min: parse_f64_or(&self.fields.imaginary_min, defaults.imaginary_min),
            imaginary_max: parse_f64_or(&self.fields.imaginary_max, defaults.imaginary_max),
            power: self.fields.power as f64,
        };

        if !plane.has_valid_bounds() {
            plane.real_min = defaults.real_min;
            plane.real_max = defaults.real_max;
            plane.imaginary_min = defaults.imaginary_min;
            plane.imaginary_max = defaults.imaginary_max;
        }

        plane
    }

    fn bounds_label(&self) -> String {
        if let Some(bounds) = self.pending_bounds {
            format!("Pending {}", format_bounds(bounds))
        } else {
            format!(
                "Current {}",
                format_bounds(PlaneBounds {
                    real_min: parse_f64_or(&self.fields.real_min, -2.0),
                    real_max: parse_f64_or(&self.fields.real_max, 2.0),
                    imaginary_min: parse_f64_or(&self.fields.imaginary_min, -2.0),
                    imaginary_max: parse_f64_or(&self.fields.imaginary_max, 2.0),
                })
            )
        }
    }

    fn set_bounds(&mut self, bounds: PlaneBounds) {
        self.fields.set_bounds(bounds);
    }

    fn run_controls(&self) -> Element<'_, Message> {
        column![
            self.flat_section("Render", self.transport_row()),
            self.flat_section("Tone", self.gamma_control()),
            self.flat_section("Output", self.save_stack()),
        ]
        .spacing(18)
        .into()
    }

    fn flat_section<'a>(
        &'a self,
        title: &'static str,
        body: Element<'a, Message>,
    ) -> Element<'a, Message> {
        column![text(title).size(12), body].spacing(7).into()
    }

    fn transport_row(&self) -> Element<'_, Message> {
        row![
            self.play_button(None, Length::Fill),
            self.pause_button(None, Length::Fill),
            self.stop_button(None, Length::Fill),
        ]
        .spacing(8)
        .into()
    }

    fn save_stack(&self) -> Element<'_, Message> {
        self.save_button(Some("Save PNG"), Length::Fill)
    }

    fn play_button(&self, label: Option<&'static str>, width: Length) -> Element<'_, Message> {
        let style = app_style();
        let running = self
            .job
            .as_ref()
            .map(|job| !job.is_finished())
            .unwrap_or(false);
        let paused = self
            .job
            .as_ref()
            .map(|job| job.is_paused())
            .unwrap_or(false);
        let button = button(action_button_content(ICON_PLAY, label))
            .width(width)
            .height(Length::Fixed(38.0))
            .padding(8)
            .style(move |_theme, status| action_button_style(style, ActionRole::Primary, status));

        if !running || paused {
            button.on_press(Message::PlayPressed).into()
        } else {
            button.into()
        }
    }

    fn pause_button(&self, label: Option<&'static str>, width: Length) -> Element<'_, Message> {
        let style = app_style();
        let running = self
            .job
            .as_ref()
            .map(|job| !job.is_finished())
            .unwrap_or(false);
        let paused = self
            .job
            .as_ref()
            .map(|job| job.is_paused())
            .unwrap_or(false);
        let button = button(action_button_content(ICON_PAUSE, label))
            .width(width)
            .height(Length::Fixed(38.0))
            .padding(8)
            .style(move |_theme, status| action_button_style(style, ActionRole::Neutral, status));

        if running && !paused {
            button.on_press(Message::PausePressed).into()
        } else {
            button.into()
        }
    }

    fn stop_button(&self, label: Option<&'static str>, width: Length) -> Element<'_, Message> {
        let style = app_style();
        let running = self
            .job
            .as_ref()
            .map(|job| !job.is_finished())
            .unwrap_or(false);
        let button = button(action_button_content(ICON_STOP, label))
            .width(width)
            .height(Length::Fixed(38.0))
            .padding(8)
            .style(move |_theme, status| action_button_style(style, ActionRole::Danger, status));

        if running {
            button.on_press(Message::StopPressed).into()
        } else {
            button.into()
        }
    }

    fn save_button(&self, label: Option<&'static str>, width: Length) -> Element<'_, Message> {
        let style = app_style();
        let button = button(action_button_content(ICON_SAVE, label))
            .width(width)
            .height(Length::Fixed(38.0))
            .padding(8)
            .style(move |_theme, status| action_button_style(style, ActionRole::Save, status));

        if self.job.is_some() {
            button.on_press(Message::SavePressed).into()
        } else {
            button.into()
        }
    }

    fn has_active_render(&self) -> bool {
        self.job
            .as_ref()
            .map(|job| !job.is_finished())
            .unwrap_or(false)
    }

    fn play_render(&mut self) {
        match &self.job {
            Some(job) if !job.is_finished() && job.is_paused() => {
                job.resume();
                self.status = String::from("Rendering...");
            }
            Some(job) if !job.is_finished() => {
                self.status = String::from("Render is already running.");
            }
            _ => self.start_render(),
        }
    }

    fn pause_render(&mut self) {
        match &self.job {
            Some(job) if !job.is_finished() && !job.is_paused() => {
                job.pause();
                self.status = String::from("Paused.");
            }
            Some(job) if job.is_paused() => {
                self.status = String::from("Render is already paused.");
            }
            _ => {
                self.status = String::from("No render is running.");
            }
        }
    }

    fn start_render(&mut self) {
        let mut fields = self.fields.clone();
        if let Some(bounds) = self.pending_bounds {
            fields.set_bounds(bounds);
        }

        match fields.parse() {
            Ok(settings) => {
                if let Some(job) = &self.job {
                    job.cancel();
                }

                self.fields = fields;
                self.pending_bounds = None;
                self.preview = PreviewImage::blank(settings.resolution);
                self.preview_plane = CPlane::from_settings(&settings);
                self.status = format!(
                    "Rendering {} samples with {} workers.",
                    settings.total_samples(),
                    settings.worker_count
                );
                self.job = Some(RenderJob::start(settings));
            }
            Err(error) => {
                self.status = error;
            }
        }
    }

    fn stop_render(&mut self) {
        if let Some(job) = &self.job {
            job.cancel();
            self.status = String::from("Cancelling render...");
        } else {
            self.status = String::from("No render is running.");
        }
    }

    fn extend_samples(&mut self) {
        let Some(job) = &mut self.job else {
            self.status = String::from("No render has been started yet.");
            return;
        };

        match job.extend_samples() {
            Ok(total_samples) => {
                self.refresh_preview();
                self.status = format!("Extended target to {total_samples} samples.");
            }
            Err(error) => self.status = error,
        }
    }

    fn save_current_image(&mut self) {
        let Some(job) = &self.job else {
            self.status = String::from("No render has been started yet.");
            return;
        };

        let file_name = match self.output_file_name() {
            Ok(file_name) => file_name,
            Err(error) => {
                self.status = error;
                return;
            }
        };

        let mut dialog = rfd::FileDialog::new().set_title("Choose PNG output folder");
        if let Some(directory) = &self.last_save_dir {
            dialog = dialog.set_directory(directory);
        }

        let Some(directory) = dialog.pick_folder() else {
            self.status = String::from("Save cancelled.");
            return;
        };

        let path = directory.join(file_name);

        match job.save_png(&path, self.tone_mapping()) {
            Ok(()) => {
                self.last_save_dir = Some(directory);
                self.status = format!("Saved {}.", path.display());
            }
            Err(error) => self.status = format!("Save failed: {error}"),
        }
    }

    fn output_file_name(&self) -> Result<String, String> {
        let trimmed = self.output_file_name.trim();
        if trimmed.is_empty() {
            return Err(String::from("PNG file name cannot be empty."));
        }

        let file_name = Path::new(trimmed)
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| String::from("PNG file name is not valid."))?;

        let mut file_name = file_name.to_owned();
        if !file_name.to_ascii_lowercase().ends_with(".png") {
            file_name.push_str(".png");
        }

        Ok(file_name)
    }

    fn tone_mapping(&self) -> ToneMapping {
        ToneMapping::new(self.gamma as f64)
    }

    fn refresh_preview(&mut self) {
        match &self.job {
            Some(_) => {}
            None => return,
        };

        let Some(job) = &mut self.job else {
            return;
        };
        let snapshot = job.snapshot(ToneMapping::new(self.gamma as f64));
        self.preview = PreviewImage::from_snapshot(snapshot);

        if job.is_finished() {
            job.join_if_finished();
            if job.was_cancelled() {
                self.status = String::from("Render cancelled.");
            } else {
                self.status = String::from("Render complete.");
            }
        } else if job.is_paused() {
            self.status = String::from("Paused.");
        } else {
            self.status = String::from("Rendering...");
        }
    }

    fn preview_plane(&self) -> CPlane {
        if self.job.is_some() {
            self.preview_plane
        } else {
            self.c_plane()
        }
    }
}

#[derive(Debug, Clone)]
struct SettingsFields {
    workers: String,
    resolution: String,
    samples_per_pixel: String,
    power: f32,
    iterations: [String; 3],
    weights: [String; 3],
    real_min: String,
    real_max: String,
    imaginary_min: String,
    imaginary_max: String,
    preview_interval_ms: String,
}

impl SettingsFields {
    fn from_settings(settings: &RenderSettings) -> Self {
        Self {
            workers: settings.worker_count.to_string(),
            resolution: settings.resolution.to_string(),
            samples_per_pixel: settings.samples_per_pixel.to_string(),
            power: settings.power as f32,
            iterations: settings.iterations.map(|value| value.to_string()),
            weights: settings.weights.map(|value| value.to_string()),
            real_min: settings.real_min.to_string(),
            real_max: settings.real_max.to_string(),
            imaginary_min: settings.imaginary_min.to_string(),
            imaginary_max: settings.imaginary_max.to_string(),
            preview_interval_ms: settings.preview_interval_ms.to_string(),
        }
    }

    fn set_bounds(&mut self, bounds: PlaneBounds) {
        self.real_min = format!("{:.8}", bounds.real_min);
        self.real_max = format!("{:.8}", bounds.real_max);
        self.imaginary_min = format!("{:.8}", bounds.imaginary_min);
        self.imaginary_max = format!("{:.8}", bounds.imaginary_max);
    }

    fn parse(&self) -> Result<RenderSettings, String> {
        let defaults = RenderSettings::default();
        let settings = RenderSettings {
            worker_count: parse_field("Workers", &self.workers)?,
            resolution: parse_field("Resolution", &self.resolution)?,
            samples_per_pixel: parse_field("Samples per pixel", &self.samples_per_pixel)?,
            brightness: defaults.brightness,
            power: self.power as f64,
            iterations: [
                parse_field("Red iterations", &self.iterations[0])?,
                parse_field("Green iterations", &self.iterations[1])?,
                parse_field("Blue iterations", &self.iterations[2])?,
            ],
            weights: [
                parse_field("Red weight", &self.weights[0])?,
                parse_field("Green weight", &self.weights[1])?,
                parse_field("Blue weight", &self.weights[2])?,
            ],
            real_min: parse_field("Real min", &self.real_min)?,
            real_max: parse_field("Real max", &self.real_max)?,
            imaginary_min: parse_field("Imaginary min", &self.imaginary_min)?,
            imaginary_max: parse_field("Imaginary max", &self.imaginary_max)?,
            escape_radius: defaults.escape_radius,
            preview_interval_ms: parse_field("Preview interval ms", &self.preview_interval_ms)?,
        };

        settings.validate()?;

        Ok(settings)
    }
}

struct PreviewImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    completed_samples: u64,
    total_samples: u64,
}

impl PreviewImage {
    fn blank(resolution: usize) -> Self {
        let pixel_count = resolution.saturating_mul(resolution);
        let mut rgba = Vec::with_capacity(pixel_count.saturating_mul(4));
        rgba.resize(pixel_count.saturating_mul(4), 255);

        for pixel in rgba.chunks_mut(4) {
            pixel[0] = 0;
            pixel[1] = 0;
            pixel[2] = 0;
        }

        Self {
            width: resolution as u32,
            height: resolution as u32,
            rgba,
            completed_samples: 0,
            total_samples: 0,
        }
    }

    fn from_snapshot(snapshot: RenderSnapshot) -> Self {
        Self {
            width: snapshot.width,
            height: snapshot.height,
            rgba: snapshot.rgba,
            completed_samples: snapshot.completed_samples,
            total_samples: snapshot.total_samples,
        }
    }

    fn progress_label(&self) -> String {
        if self.total_samples == 0 {
            return String::from("0%");
        }

        let progress = self.completed_samples as f64 / self.total_samples as f64;
        format!("{:.1}%", (progress * 100.0).clamp(0.0, 100.0))
    }

    fn progress_fraction(&self) -> f32 {
        if self.total_samples == 0 {
            return 0.0;
        }

        (self.completed_samples as f64 / self.total_samples as f64).clamp(0.0, 1.0) as f32
    }
}

struct PreviewSurface {
    handle: iced::widget::image::Handle,
    width: u32,
    height: u32,
    plane: CPlane,
    pending_bounds: Option<PlaneBounds>,
    show_outline: bool,
    background: Color,
    border: Color,
}

struct PreviewSurfaceState {
    zoom: f32,
    pan: Vector,
    drag: PreviewDrag,
    last_cursor: Option<Point>,
    selection_start: Option<Point>,
    selection_current: Option<Point>,
    outline_key: RefCell<Option<PreviewOutlineKey>>,
    outline_handle: RefCell<Option<iced::widget::image::Handle>>,
}

impl Default for PreviewSurfaceState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vector::new(0.0, 0.0),
            drag: PreviewDrag::None,
            last_cursor: None,
            selection_start: None,
            selection_current: None,
            outline_key: RefCell::new(None),
            outline_handle: RefCell::new(None),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewDrag {
    None,
    Pan,
    Select,
}

struct SelectionOverlay {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    empty: bool,
}

impl SelectionOverlay {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            rgba: vec![0; width as usize * height as usize * 4],
            empty: true,
        }
    }

    fn is_empty(&self) -> bool {
        self.empty
    }

    fn into_rgba(self) -> Vec<u8> {
        self.rgba
    }

    fn draw_rect(&mut self, start: Point, current: Point, fill: [u8; 4], stroke: [u8; 4]) {
        let x0 = start.x.min(current.x).round() as i32;
        let y0 = start.y.min(current.y).round() as i32;
        let x1 = start.x.max(current.x).round() as i32;
        let y1 = start.y.max(current.y).round() as i32;
        let width = x1 - x0;
        let height = y1 - y0;

        self.empty = false;

        if width <= 1 || height <= 1 {
            self.draw_marker(start, stroke);
            return;
        }

        self.fill_rect(x0, y0, x1, y1, fill);
        self.stroke_rect(x0, y0, x1, y1, 7, [0, 0, 0, 230]);
        self.stroke_rect(x0, y0, x1, y1, 3, stroke);

        let handle_size = 12.min(width / 3).min(height / 3);
        if handle_size >= 3 {
            for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
                let half = handle_size / 2;
                self.fill_rect(x - half, y - half, x + half, y + half, stroke);
                self.stroke_rect(x - half, y - half, x + half, y + half, 1, [0, 0, 0, 230]);
            }
        }
    }

    fn draw_marker(&mut self, center: Point, color: [u8; 4]) {
        let x = center.x.round() as i32;
        let y = center.y.round() as i32;

        self.fill_rect(x - 5, y - 5, x + 5, y + 5, color);
        self.stroke_rect(x - 5, y - 5, x + 5, y + 5, 2, [0, 0, 0, 230]);
    }

    fn fill_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: [u8; 4]) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.set_pixel(x, y, color);
            }
        }
    }

    fn stroke_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, thickness: i32, color: [u8; 4]) {
        for offset in 0..thickness {
            for x in (x0 + offset)..=(x1 - offset) {
                self.set_pixel(x, y0 + offset, color);
                self.set_pixel(x, y1 - offset, color);
            }

            for y in (y0 + offset)..=(y1 - offset) {
                self.set_pixel(x0 + offset, y, color);
                self.set_pixel(x1 - offset, y, color);
            }
        }
    }

    fn set_pixel(&mut self, x: i32, y: i32, color: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }

        let index = (y as usize * self.width as usize + x as usize) * 4;
        self.rgba[index..index + 4].copy_from_slice(&color);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PreviewOutlineKey {
    width: u32,
    height: u32,
    real_min: f64,
    real_max: f64,
    imaginary_min: f64,
    imaginary_max: f64,
    power: f64,
}

impl PreviewSurface {
    fn local_bounds(bounds: Rectangle) -> Rectangle {
        Rectangle::new(Point::ORIGIN, bounds.size())
    }

    fn image_bounds(&self, bounds: Rectangle, state: &PreviewSurfaceState) -> Rectangle {
        let image_width = self.width.max(1) as f32;
        let image_height = self.height.max(1) as f32;
        let base_scale = (bounds.width / image_width)
            .min(bounds.height / image_height)
            .max(0.001);
        let scale = base_scale * state.zoom;
        let width = image_width * scale;
        let height = image_height * scale;

        Rectangle::new(
            Point::new(
                bounds.x + (bounds.width - width) / 2.0 + state.pan.x,
                bounds.y + (bounds.height - height) / 2.0 + state.pan.y,
            ),
            Size::new(width, height),
        )
    }

    fn outline_size(&self) -> (u32, u32) {
        let width = self.width.max(8);
        let height = self.height.max(8);
        let longest = width.max(height);

        if longest <= OUTLINE_MAX_RESOLUTION {
            (width, height)
        } else {
            let scale = OUTLINE_MAX_RESOLUTION as f32 / longest as f32;
            (
                ((width as f32 * scale).round() as u32).max(8),
                ((height as f32 * scale).round() as u32).max(8),
            )
        }
    }

    fn point_to_overlay(
        point: Point,
        image_area: Rectangle,
        overlay_width: u32,
        overlay_height: u32,
    ) -> Point {
        Point::new(
            ((point.x - image_area.x) / image_area.width).clamp(0.0, 1.0) * overlay_width as f32,
            ((point.y - image_area.y) / image_area.height).clamp(0.0, 1.0) * overlay_height as f32,
        )
    }

    fn selection_overlay_handle(
        &self,
        state: &PreviewSurfaceState,
        image_area: Rectangle,
    ) -> Option<iced::widget::image::Handle> {
        let (overlay_width, overlay_height) = self.outline_size();
        let mut overlay = SelectionOverlay::new(overlay_width, overlay_height);

        if let Some(bounds) = self.pending_bounds {
            let start =
                self.plane
                    .complex_to_position(bounds.real_min, bounds.imaginary_min, image_area);
            let current =
                self.plane
                    .complex_to_position(bounds.real_max, bounds.imaginary_max, image_area);

            overlay.draw_rect(
                Self::point_to_overlay(start, image_area, overlay_width, overlay_height),
                Self::point_to_overlay(current, image_area, overlay_width, overlay_height),
                [0, 0, 0, 56],
                [255, 224, 102, 255],
            );
        }

        if let (Some(start), Some(current)) = (state.selection_start, state.selection_current) {
            overlay.draw_rect(
                Self::point_to_overlay(start, image_area, overlay_width, overlay_height),
                Self::point_to_overlay(current, image_area, overlay_width, overlay_height),
                [64, 174, 224, 70],
                [255, 255, 255, 255],
            );
        }

        if overlay.is_empty() {
            None
        } else {
            Some(iced::widget::image::Handle::from_rgba(
                overlay_width,
                overlay_height,
                overlay.into_rgba(),
            ))
        }
    }

    fn aspect_ratio(&self) -> f32 {
        self.width.max(1) as f32 / self.height.max(1) as f32
    }

    fn clamp_pan(&self, bounds: Rectangle, state: &mut PreviewSurfaceState) {
        if state.zoom <= 1.0 {
            state.pan = Vector::new(0.0, 0.0);
            return;
        }

        let image_bounds = self.image_bounds(bounds, state);
        let max_x = ((image_bounds.width - bounds.width) / 2.0).max(0.0) + bounds.width * 0.25;
        let max_y = ((image_bounds.height - bounds.height) / 2.0).max(0.0) + bounds.height * 0.25;

        state.pan = Vector::new(
            state.pan.x.clamp(-max_x, max_x),
            state.pan.y.clamp(-max_y, max_y),
        );
    }

    fn clear_drag(state: &mut PreviewSurfaceState) {
        state.drag = PreviewDrag::None;
        state.last_cursor = None;
        state.selection_start = None;
        state.selection_current = None;
    }

    fn constrain_selection_to_aspect(
        start: Point,
        current: Point,
        area: Rectangle,
        aspect_ratio: f32,
    ) -> Point {
        let current = Point::new(
            current.x.clamp(area.x, area.x + area.width),
            current.y.clamp(area.y, area.y + area.height),
        );
        let dx = current.x - start.x;
        let dy = current.y - start.y;

        if dx.abs() <= 1.0 || dy.abs() <= 1.0 {
            return current;
        }

        let mut width = dx.abs();
        let mut height = dy.abs();
        let aspect_ratio = aspect_ratio.max(0.001);

        if width / height > aspect_ratio {
            width = height * aspect_ratio;
        } else {
            height = width / aspect_ratio;
        }

        Point::new(
            start.x + dx.signum() * width,
            start.y + dy.signum() * height,
        )
    }

    fn draw_selection_rect(
        frame: &mut canvas::Frame,
        start: Point,
        current: Point,
        fill: Color,
        stroke: Color,
    ) {
        let x = start.x.min(current.x);
        let y = start.y.min(current.y);
        let width = (current.x - start.x).abs();
        let height = (current.y - start.y).abs();

        if width <= 1.0 || height <= 1.0 {
            let marker = canvas::Path::circle(start, 5.0);
            frame.fill(&marker, stroke);
            frame.stroke(
                &marker,
                canvas::Stroke::default()
                    .with_color(Color::from_rgba8(0, 0, 0, 0.90))
                    .with_width(2.0),
            );
            return;
        }

        let path = canvas::Path::rectangle(Point::new(x, y), Size::new(width, height));
        frame.fill(&path, fill);
        frame.stroke(
            &path,
            canvas::Stroke::default()
                .with_color(Color::from_rgba8(0, 0, 0, 0.90))
                .with_width(5.0),
        );
        frame.stroke(
            &path,
            canvas::Stroke::default().with_color(stroke).with_width(2.0),
        );

        let handle_size = 8.0_f32.min(width / 3.0).min(height / 3.0);
        if handle_size < 2.0 {
            return;
        }

        for point in [
            Point::new(x, y),
            Point::new(x + width, y),
            Point::new(x, y + height),
            Point::new(x + width, y + height),
        ] {
            let handle = canvas::Path::rectangle(
                Point::new(point.x - handle_size / 2.0, point.y - handle_size / 2.0),
                Size::new(handle_size, handle_size),
            );
            frame.fill(&handle, stroke);
            frame.stroke(
                &handle,
                canvas::Stroke::default()
                    .with_color(Color::from_rgba8(0, 0, 0, 0.90))
                    .with_width(1.0),
            );
        }
    }

    fn draw_pending_bounds(&self, frame: &mut canvas::Frame, area: Rectangle) {
        let Some(bounds) = self.pending_bounds else {
            return;
        };

        let start = self
            .plane
            .complex_to_position(bounds.real_min, bounds.imaginary_min, area);
        let current = self
            .plane
            .complex_to_position(bounds.real_max, bounds.imaginary_max, area);

        Self::draw_selection_rect(
            frame,
            start,
            current,
            Color::from_rgba8(0, 0, 0, 0.20),
            Color::from_rgb8(255, 224, 102),
        );
    }

    fn draw_drag_selection(&self, state: &PreviewSurfaceState, frame: &mut canvas::Frame) {
        let (Some(start), Some(current)) = (state.selection_start, state.selection_current) else {
            return;
        };

        Self::draw_selection_rect(
            frame,
            start,
            current,
            Color::from_rgba8(64, 174, 224, 0.24),
            Color::from_rgb8(255, 255, 255),
        );
    }
}

impl canvas::Program<Message> for PreviewSurface {
    type State = PreviewSurfaceState;

    fn update(
        &self,
        state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (canvas::event::Status, Option<Message>) {
        let local_bounds = Self::local_bounds(bounds);
        let image_area = self.image_bounds(local_bounds, state);

        match event {
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if cursor.position_in(bounds).is_none() {
                    return (canvas::event::Status::Ignored, None);
                }

                let y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => y,
                    mouse::ScrollDelta::Pixels { y, .. } => y / 48.0,
                };

                if y.abs() <= f32::EPSILON {
                    return (canvas::event::Status::Ignored, None);
                }

                let old_zoom = state.zoom;
                let zoom_factor = 1.12_f32.powf(y);
                state.zoom = (state.zoom * zoom_factor).clamp(1.0, 32.0);

                if let Some(position) = cursor.position_in(bounds) {
                    let ratio = if old_zoom > 0.0 {
                        state.zoom / old_zoom
                    } else {
                        1.0
                    };
                    let center = Point::new(local_bounds.width / 2.0, local_bounds.height / 2.0);
                    state.pan = Vector::new(
                        (state.pan.x + center.x - position.x) * ratio - (center.x - position.x),
                        (state.pan.y + center.y - position.y) * ratio - (center.y - position.y),
                    );
                }

                self.clamp_pan(local_bounds, state);
                (
                    canvas::event::Status::Captured,
                    Some(Message::PreviewInteractionChanged),
                )
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(position) = cursor
                    .position_in(bounds)
                    .filter(|position| image_area.contains(*position))
                else {
                    return (canvas::event::Status::Ignored, None);
                };

                state.drag = PreviewDrag::Select;
                state.last_cursor = Some(position);
                state.selection_start = Some(position);
                state.selection_current = Some(position);
                (
                    canvas::event::Status::Captured,
                    Some(Message::PreviewInteractionChanged),
                )
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Right | mouse::Button::Middle,
            )) => {
                let Some(position) = cursor.position_in(bounds) else {
                    return (canvas::event::Status::Ignored, None);
                };

                state.drag = PreviewDrag::Pan;
                state.last_cursor = Some(position);
                (
                    canvas::event::Status::Captured,
                    Some(Message::PreviewInteractionChanged),
                )
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if state.drag == PreviewDrag::None {
                    return (canvas::event::Status::Ignored, None);
                }

                let position = position - Vector::new(bounds.x, bounds.y);

                match state.drag {
                    PreviewDrag::Pan => {
                        if let Some(previous) = state.last_cursor {
                            state.pan = Vector::new(
                                state.pan.x + position.x - previous.x,
                                state.pan.y + position.y - previous.y,
                            );
                            self.clamp_pan(local_bounds, state);
                        }
                    }
                    PreviewDrag::Select => {
                        if let Some(start) = state.selection_start {
                            state.selection_current = Some(Self::constrain_selection_to_aspect(
                                start,
                                position,
                                image_area,
                                self.aspect_ratio(),
                            ));
                        }
                    }
                    PreviewDrag::None => {}
                }

                state.last_cursor = Some(position);
                (
                    canvas::event::Status::Captured,
                    Some(Message::PreviewInteractionChanged),
                )
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.drag != PreviewDrag::Select {
                    return (canvas::event::Status::Ignored, None);
                }

                let start = state.selection_start;
                let current = state.selection_current;
                Self::clear_drag(state);

                let (Some(start), Some(current)) = (start, current) else {
                    return (
                        canvas::event::Status::Captured,
                        Some(Message::PreviewInteractionChanged),
                    );
                };

                let width = (current.x - start.x).abs();
                let height = (current.y - start.y).abs();
                if width <= 4.0 || height <= 4.0 {
                    return (
                        canvas::event::Status::Captured,
                        Some(Message::PreviewInteractionChanged),
                    );
                }

                (
                    canvas::event::Status::Captured,
                    Some(Message::BoundsSelected(
                        self.plane.selection_to_bounds(start, current, image_area),
                    )),
                )
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(
                mouse::Button::Right | mouse::Button::Middle,
            )) => {
                if state.drag != PreviewDrag::Pan {
                    return (canvas::event::Status::Ignored, None);
                }

                Self::clear_drag(state);
                (
                    canvas::event::Status::Captured,
                    Some(Message::PreviewInteractionChanged),
                )
            }
            _ => (canvas::event::Status::Ignored, None),
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let local_bounds = Self::local_bounds(bounds);
        let background = canvas::Path::rectangle(local_bounds.position(), local_bounds.size());
        let image_area = self.image_bounds(local_bounds, state);
        let (outline_width, outline_height) = self.outline_size();
        let outline_key = PreviewOutlineKey {
            width: outline_width,
            height: outline_height,
            real_min: self.plane.real_min,
            real_max: self.plane.real_max,
            imaginary_min: self.plane.imaginary_min,
            imaginary_max: self.plane.imaginary_max,
            power: self.plane.power,
        };

        if self.show_outline && state.outline_key.borrow().as_ref() != Some(&outline_key) {
            let rgba = self
                .plane
                .outline_overlay_rgba(outline_width, outline_height);
            *state.outline_handle.borrow_mut() = Some(iced::widget::image::Handle::from_rgba(
                outline_width,
                outline_height,
                rgba,
            ));
            *state.outline_key.borrow_mut() = Some(outline_key);
        }

        frame.fill(&background, self.background);
        frame.with_clip(local_bounds, |frame| {
            frame.draw_image(image_area, &self.handle);
            if self.show_outline {
                if let Some(handle) = state.outline_handle.borrow().as_ref() {
                    frame.draw_image(image_area, handle);
                }
                self.plane.draw_axes(frame, image_area);
            }
            if let Some(handle) = self.selection_overlay_handle(state, image_area) {
                frame.draw_image(image_area, &handle);
            }
        });
        frame.stroke(
            &background,
            canvas::Stroke::default()
                .with_color(self.border)
                .with_width(1.0),
        );

        let mut overlay = canvas::Frame::new(renderer, bounds.size());
        overlay.with_clip(local_bounds, |frame| {
            self.draw_pending_bounds(frame, image_area);
            self.draw_drag_selection(state, frame);
        });

        vec![frame.into_geometry(), overlay.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.drag == PreviewDrag::Pan {
            mouse::Interaction::Grabbing
        } else if state.drag == PreviewDrag::Select {
            mouse::Interaction::Crosshair
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

fn field<'a, F>(label: &'a str, value: &'a str, on_input: F) -> Element<'a, Message>
where
    F: Fn(String) -> Message + 'a,
{
    column![
        text(label).size(12),
        text_input("", value).on_input(on_input).padding(8)
    ]
    .spacing(4)
    .into()
}

fn tab_button(
    label: &'static str,
    tab: ControlsTab,
    active_tab: ControlsTab,
) -> Element<'static, Message> {
    let label = if tab == active_tab {
        format!("[{label}]")
    } else {
        label.to_owned()
    };

    button(text(label))
        .on_press(Message::TabSelected(tab))
        .width(Length::Fill)
        .into()
}

fn channel_fields<'a, IterMessage, WeightMessage>(
    name: &'a str,
    iterations: &'a str,
    on_iterations: IterMessage,
    weight: &'a str,
    on_weight: WeightMessage,
) -> Element<'a, Message>
where
    IterMessage: Fn(String) -> Message + 'a,
    WeightMessage: Fn(String) -> Message + 'a,
{
    column![
        text(name).size(13),
        row![
            field("Iterations", iterations, on_iterations),
            field("Weight", weight, on_weight)
        ]
        .spacing(8)
    ]
    .spacing(4)
    .into()
}

fn recommended_samples_per_pixel(resolution: usize) -> u16 {
    match resolution {
        0..=256 => 100,
        257..=450 => DEFAULT_SAMPLES_PER_PIXEL as u16,
        451..=720 => 350,
        721..=1080 => 550,
        1081..=1440 => 800,
        1441..=2160 => 1_200,
        _ => 2_000,
    }
}

fn parse_field<T>(label: &str, value: &str) -> Result<T, String>
where
    T: std::str::FromStr,
{
    value
        .trim()
        .parse::<T>()
        .map_err(|_| format!("{label} is not a valid value."))
}

fn parse_f64_or(value: &str, fallback: f64) -> f64 {
    value.trim().parse::<f64>().unwrap_or(fallback)
}

fn parse_u16_or(value: &str, fallback: u16) -> u16 {
    value.trim().parse::<u16>().unwrap_or(fallback)
}

#[derive(Debug, Clone, Copy)]
struct PlaneBounds {
    real_min: f64,
    real_max: f64,
    imaginary_min: f64,
    imaginary_max: f64,
}

fn format_bounds(bounds: PlaneBounds) -> String {
    format!(
        "Re {:.3}..{:.3}, Im {:.3}..{:.3}",
        bounds.real_min, bounds.real_max, bounds.imaginary_min, bounds.imaginary_max
    )
}

#[derive(Debug, Clone, Copy)]
struct CPlane {
    real_min: f64,
    real_max: f64,
    imaginary_min: f64,
    imaginary_max: f64,
    power: f64,
}

impl CPlane {
    fn from_settings(settings: &RenderSettings) -> Self {
        Self {
            real_min: settings.real_min,
            real_max: settings.real_max,
            imaginary_min: settings.imaginary_min,
            imaginary_max: settings.imaginary_max,
            power: settings.power,
        }
    }

    fn has_valid_bounds(&self) -> bool {
        self.real_min.is_finite()
            && self.real_max.is_finite()
            && self.imaginary_min.is_finite()
            && self.imaginary_max.is_finite()
            && self.real_min < self.real_max
            && self.imaginary_min < self.imaginary_max
    }

    fn complex_to_position(&self, real: f64, imaginary: f64, area: Rectangle) -> Point {
        let x = ((real - self.real_min) / (self.real_max - self.real_min)).clamp(0.0, 1.0);
        let y = ((imaginary - self.imaginary_min) / (self.imaginary_max - self.imaginary_min))
            .clamp(0.0, 1.0);

        Point::new(
            area.x + area.width * x as f32,
            area.y + area.height * y as f32,
        )
    }

    fn position_to_complex(&self, position: Point, area: Rectangle) -> (f64, f64) {
        let x = ((position.x - area.x) / area.width).clamp(0.0, 1.0) as f64;
        let y = ((position.y - area.y) / area.height).clamp(0.0, 1.0) as f64;

        (
            self.real_min + x * (self.real_max - self.real_min),
            self.imaginary_min + y * (self.imaginary_max - self.imaginary_min),
        )
    }

    fn selection_to_bounds(&self, start: Point, end: Point, area: Rectangle) -> PlaneBounds {
        let (start_real, start_imaginary) = self.position_to_complex(start, area);
        let (end_real, end_imaginary) = self.position_to_complex(end, area);

        PlaneBounds {
            real_min: start_real.min(end_real),
            real_max: start_real.max(end_real),
            imaginary_min: start_imaginary.min(end_imaginary),
            imaginary_max: start_imaginary.max(end_imaginary),
        }
    }
}

impl CPlane {
    fn outline_overlay_rgba(&self, width: u32, height: u32) -> Vec<u8> {
        const MAX_ITERATIONS: usize = 72;
        let width = width.max(8) as usize;
        let height = height.max(8) as usize;

        let mut inside = vec![false; width * height];
        let mut rgba = vec![0; width * height * 4];

        for y in 0..height {
            for x in 0..width {
                let real = self.real_min
                    + (x as f64 + 0.5) / width as f64 * (self.real_max - self.real_min);
                let imaginary = self.imaginary_min
                    + (y as f64 + 0.5) / height as f64 * (self.imaginary_max - self.imaginary_min);

                inside[y * width + x] =
                    mandelbrot_contains(real, imaginary, self.power, MAX_ITERATIONS);
            }
        }

        for y in 0..height {
            for x in 0..width {
                if !inside[y * width + x] {
                    continue;
                }

                let on_edge = x == 0
                    || y == 0
                    || x == width - 1
                    || y == height - 1
                    || !inside[y * width + x - 1]
                    || !inside[y * width + x + 1]
                    || !inside[(y - 1) * width + x]
                    || !inside[(y + 1) * width + x];

                let pixel = (y * width + x) * 4;
                if on_edge {
                    rgba[pixel] = 78;
                    rgba[pixel + 1] = 212;
                    rgba[pixel + 2] = 255;
                    rgba[pixel + 3] = 235;
                } else {
                    rgba[pixel] = 78;
                    rgba[pixel + 1] = 212;
                    rgba[pixel + 2] = 255;
                    rgba[pixel + 3] = 24;
                }
            }
        }

        rgba
    }

    fn draw_axes(&self, frame: &mut canvas::Frame, area: Rectangle) {
        let axis = canvas::Stroke::default()
            .with_color(Color::from_rgba8(255, 255, 255, 0.18))
            .with_width(1.0);

        if self.real_min < 0.0 && self.real_max > 0.0 {
            let origin = self.complex_to_position(0.0, 0.0, area);
            let path = canvas::Path::line(
                Point::new(origin.x, area.y),
                Point::new(origin.x, area.y + area.height),
            );
            frame.stroke(&path, axis);
        }

        if self.imaginary_min < 0.0 && self.imaginary_max > 0.0 {
            let origin = self.complex_to_position(0.0, 0.0, area);
            let path = canvas::Path::line(
                Point::new(area.x, origin.y),
                Point::new(area.x + area.width, origin.y),
            );
            frame.stroke(&path, axis);
        }
    }
}

fn mandelbrot_contains(real: f64, imaginary: f64, power: f64, max_iterations: usize) -> bool {
    let c = PlaneComplex { real, imaginary };
    let mut z = PlaneComplex::ZERO;
    let power = PreviewPower::from(power);

    for _ in 0..max_iterations {
        if z.length_squared() > 4.0 {
            return false;
        }

        z = z.pow(power) + c;
    }

    true
}

#[derive(Debug, Clone, Copy)]
struct PlaneComplex {
    real: f64,
    imaginary: f64,
}

impl PlaneComplex {
    const ZERO: Self = Self {
        real: 0.0,
        imaginary: 0.0,
    };

    fn length_squared(self) -> f64 {
        self.real * self.real + self.imaginary * self.imaginary
    }

    fn pow(self, power: PreviewPower) -> Self {
        match power {
            PreviewPower::One => self,
            PreviewPower::Two => self * self,
            PreviewPower::Integer(exponent) => self.pow_integer(exponent),
            PreviewPower::Real(power) => self.pow_real(power),
        }
    }

    fn pow_integer(self, exponent: u32) -> Self {
        let mut result = Self {
            real: 1.0,
            imaginary: 0.0,
        };
        let mut base = self;
        let mut exponent = exponent;

        while exponent > 0 {
            if exponent % 2 == 1 {
                result = result * base;
            }

            base = base * base;
            exponent /= 2;
        }

        result
    }

    fn pow_real(self, power: f64) -> Self {
        if self.real == 0.0 && self.imaginary == 0.0 {
            return Self::ZERO;
        }

        let radius = self.length_squared().sqrt().powf(power);
        let angle = self.imaginary.atan2(self.real) * power;

        Self {
            real: radius * angle.cos(),
            imaginary: radius * angle.sin(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum PreviewPower {
    One,
    Two,
    Integer(u32),
    Real(f64),
}

impl From<f64> for PreviewPower {
    fn from(value: f64) -> Self {
        if (value - 1.0).abs() < f64::EPSILON {
            Self::One
        } else if (value - 2.0).abs() < f64::EPSILON {
            Self::Two
        } else {
            let rounded = value.round();
            if (value - rounded).abs() < f64::EPSILON && rounded >= 1.0 {
                Self::Integer(rounded as u32)
            } else {
                Self::Real(value)
            }
        }
    }
}

impl std::ops::Mul for PlaneComplex {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            real: self.real * rhs.real - self.imaginary * rhs.imaginary,
            imaginary: self.real * rhs.imaginary + self.imaginary * rhs.real,
        }
    }
}

impl std::ops::Add for PlaneComplex {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            real: self.real + rhs.real,
            imaginary: self.imaginary + rhs.imaginary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_outline_overlay_has_visible_pixels() {
        let settings = RenderSettings::default();
        let plane = CPlane {
            real_min: settings.real_min,
            real_max: settings.real_max,
            imaginary_min: settings.imaginary_min,
            imaginary_max: settings.imaginary_max,
            power: settings.power,
        };

        let rgba = plane.outline_overlay_rgba(128, 128);
        let visible_pixels = rgba.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
        let strong_edge_pixels = rgba.chunks_exact(4).filter(|pixel| pixel[3] > 200).count();

        assert_eq!(rgba.len(), 128 * 128 * 4);
        assert!(visible_pixels > 0);
        assert!(strong_edge_pixels > 0);
    }

    #[test]
    fn preview_plane_y_mapping_matches_renderer_direction() {
        let plane = CPlane {
            real_min: -2.0,
            real_max: 2.0,
            imaginary_min: -3.0,
            imaginary_max: 5.0,
            power: 2.0,
        };
        let area = Rectangle::new(Point::ORIGIN, Size::new(100.0, 100.0));

        let top = plane.position_to_complex(Point::new(50.0, 0.0), area);
        let bottom = plane.position_to_complex(Point::new(50.0, 100.0), area);
        let imaginary_min = plane.complex_to_position(0.0, -3.0, area);
        let imaginary_max = plane.complex_to_position(0.0, 5.0, area);

        assert_eq!(top.1, -3.0);
        assert_eq!(bottom.1, 5.0);
        assert_eq!(imaginary_min.y, 0.0);
        assert_eq!(imaginary_max.y, 100.0);
    }

    #[test]
    fn selection_is_constrained_to_preview_aspect_ratio() {
        let area = Rectangle::new(Point::ORIGIN, Size::new(100.0, 100.0));
        let start = Point::new(10.0, 10.0);
        let current = Point::new(90.0, 50.0);

        let constrained = PreviewSurface::constrain_selection_to_aspect(start, current, area, 1.0);
        let width = (constrained.x - start.x).abs();
        let height = (constrained.y - start.y).abs();

        assert!((width - height).abs() < f32::EPSILON);
    }

    #[test]
    fn selection_uses_current_cropped_preview_plane() {
        let plane = CPlane {
            real_min: -1.0,
            real_max: 1.0,
            imaginary_min: -0.5,
            imaginary_max: 0.5,
            power: 2.0,
        };
        let area = Rectangle::new(Point::ORIGIN, Size::new(100.0, 100.0));

        let selected =
            plane.selection_to_bounds(Point::new(25.0, 25.0), Point::new(75.0, 75.0), area);

        assert_eq!(selected.real_min, -0.5);
        assert_eq!(selected.real_max, 0.5);
        assert_eq!(selected.imaginary_min, -0.25);
        assert_eq!(selected.imaginary_max, 0.25);
    }

    #[test]
    fn recommended_samples_increase_with_resolution() {
        assert_eq!(recommended_samples_per_pixel(256), 100);
        assert_eq!(
            recommended_samples_per_pixel(450),
            DEFAULT_SAMPLES_PER_PIXEL as u16
        );
        assert!(recommended_samples_per_pixel(4096) > recommended_samples_per_pixel(1080));
        assert!(recommended_samples_per_pixel(4096) <= SAMPLE_SLIDER_MAX);
    }

    #[test]
    fn app_defaults_to_adjusted_gamma() {
        let app = BudahbrotApp::new();

        assert_eq!(app.gamma, DEFAULT_GAMMA);
        assert_eq!(app.tone_mapping().gamma, DEFAULT_GAMMA as f64);
    }
}
