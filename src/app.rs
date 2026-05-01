use std::{collections::HashMap, fs, path::PathBuf, time::Instant};

use crossterm::event::KeyCode;
use ratatui::style::Color;
use ttf_parser::name_id;

use crate::font::{load_font, save_fixed_font};

pub const ACCENT: Color = Color::Magenta;
pub const CURSOR_BLINK_MS: u128 = 500;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    Family,
    Subfamily,
    UniqueId,
    FullName,
    Version,
    PostScriptName,
    PreferredFamily,
    PreferredSubfamily,
    CompatibleFullName,
    WwsFamily,
    WwsSubfamily,
    VariationsPostScriptPrefix,
    WeightClass,
    WidthClass,
    BoldFlag,
    ItalicFlag,
    ObliqueFlag,
    RegularFlag,
    Copyright,
    Trademark,
    Manufacturer,
    Designer,
    Description,
    LicenseDescription,
    LicenseUrl,
    VendorUrl,
    DesignerUrl,
}

pub const FIELDS: [Field; 27] = [
    Field::Family,
    Field::Subfamily,
    Field::UniqueId,
    Field::FullName,
    Field::Version,
    Field::PostScriptName,
    Field::PreferredFamily,
    Field::PreferredSubfamily,
    Field::CompatibleFullName,
    Field::WwsFamily,
    Field::WwsSubfamily,
    Field::VariationsPostScriptPrefix,
    Field::Copyright,
    Field::Trademark,
    Field::Manufacturer,
    Field::Designer,
    Field::Description,
    Field::LicenseDescription,
    Field::LicenseUrl,
    Field::VendorUrl,
    Field::DesignerUrl,
    Field::WeightClass,
    Field::WidthClass,
    Field::BoldFlag,
    Field::ItalicFlag,
    Field::ObliqueFlag,
    Field::RegularFlag,
];

#[derive(Clone, PartialEq)]
pub struct FontInfo(HashMap<Field, String>);

impl Default for FontInfo {
    fn default() -> Self {
        let mut map = HashMap::new();
        for field in FIELDS {
            let val = match field {
                Field::WeightClass => "400",
                Field::WidthClass => "5",
                Field::BoldFlag | Field::ItalicFlag | Field::ObliqueFlag | Field::RegularFlag => "false",
                _ => "",
            };
            map.insert(field, val.to_string());
        }
        Self(map)
    }
}

impl FontInfo {
    pub fn get(&self, field: Field) -> String {
        self.0.get(&field).cloned().unwrap_or_default()
    }

    pub fn get_bool(&self, field: Field) -> bool {
        self.0.get(&field).is_some_and(|v| v == "true")
    }

    pub fn get_u16(&self, field: Field) -> u16 {
        self.0.get(&field).and_then(|v| v.parse().ok()).unwrap_or(0)
    }

    pub fn set(&mut self, field: Field, value: String) {
        let normalized = match field {
            Field::WeightClass => value.parse::<u16>()
                .map(|v| v.clamp(100, 900).to_string())
                .unwrap_or_else(|_| self.get(field)),
            Field::WidthClass => value.parse::<u16>()
                .map(|v| v.clamp(1, 9).to_string())
                .unwrap_or_else(|_| self.get(field)),
            Field::BoldFlag | Field::ItalicFlag | Field::ObliqueFlag | Field::RegularFlag => {
                matches!(value.trim().to_lowercase().as_str(), "true" | "yes" | "y" | "1" | "on")
                    .to_string()
            }
            _ => value,
        };
        self.0.insert(field, normalized);
    }
}

pub struct FontFile {
    pub path: PathBuf,
    pub original: FontInfo,
    pub edited: FontInfo,
    pub variable: bool,
}

impl FontFile {
    pub fn has_changes(&self) -> bool {
        self.original != self.edited
    }
}

pub enum Screen {
    Editor,
    Help,
}

pub enum Focus {
    Fonts,
    Fields,
}

pub enum PendingAction {
    ApplyFieldToAll {
        field: Field,
        value: String,
        selected_choice: usize,
    },
    ConfirmQuit {
        selected_choice: usize,
    },
}

pub struct App {
    pub screen: Screen,
    pub focus: Focus,
    pub fonts: Vec<FontFile>,
    pub selected_font: usize,
    pub selected_field: usize,
    pub input: String,
    pub editing: bool,
    pub status: String,
    pub cursor_started: Instant,
    pub pending_action: Option<PendingAction>,
    pub should_quit: bool,
    pub output_dir: PathBuf,
}

impl Default for App {
    fn default() -> Self {
        Self {
            screen: Screen::Editor,
            focus: Focus::Fonts,
            fonts: vec![],
            selected_font: 0,
            selected_field: 0,
            input: String::new(),
            editing: false,
            status: "Drag font files into window to load".into(),
            cursor_started: Instant::now(),
            pending_action: None,
            should_quit: false,
            output_dir: PathBuf::from("Export"),
        }
    }
}

impl Field {
    pub fn is_editable(self) -> bool {
        !matches!(self, Field::WeightClass | Field::WidthClass | Field::BoldFlag | Field::ItalicFlag | Field::ObliqueFlag | Field::RegularFlag)
    }

    pub fn description(self) -> &'static str {
        match self {
            Field::Family => "Basic font family name used by legacy systems",
            Field::Subfamily => "Weight and style (e.g., Bold, Italic)",
            Field::UniqueId => "Unique identifier for this specific font variant",
            Field::FullName => "Complete font name including all styling",
            Field::Version => "Font version number",
            Field::PostScriptName => "Internal name used by PostScript and PDF",
            Field::PreferredFamily => "Modern family name used by newer applications",
            Field::PreferredSubfamily => "Modern style designation",
            Field::CompatibleFullName => "Alternative full name for compatibility",
            Field::WwsFamily => "Family name for the Windows weight/width/slope model",
            Field::WwsSubfamily => "Subfamily name for the Windows weight/width/slope model",
            Field::VariationsPostScriptPrefix => "Prefix for PostScript names of variable font instances",
            Field::Copyright => "Copyright notice and attribution",
            Field::Trademark => "Trademark information",
            Field::Manufacturer => "Font foundry or vendor name",
            Field::Designer => "Font designer or creator",
            Field::Description => "Description of the font characteristics",
            Field::LicenseDescription => "License terms and restrictions",
            Field::LicenseUrl => "URL to full license document",
            Field::VendorUrl => "URL for the font vendor or foundry website",
            Field::DesignerUrl => "URL for the font designer's website",
            Field::WeightClass => "Numeric weight: 100=Thin, 400=Regular, 900=Black",
            Field::WidthClass => "Numeric width: 1=Ultra-condensed, 5=Normal, 9=Ultra-expanded",
            Field::BoldFlag => "Whether font is marked as bold",
            Field::ItalicFlag => "Whether font is marked as italic",
            Field::ObliqueFlag => "Whether font is marked as oblique",
            Field::RegularFlag => "Whether font is marked as regular",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Field::Family => "Family",
            Field::Subfamily => "Subfamily",
            Field::UniqueId => "Unique ID",
            Field::FullName => "Full name",
            Field::Version => "Version",
            Field::PostScriptName => "PostScript name",
            Field::PreferredFamily => "Preferred family",
            Field::PreferredSubfamily => "Preferred subfamily",
            Field::CompatibleFullName => "Compatible full name",
            Field::WwsFamily => "WWS family",
            Field::WwsSubfamily => "WWS subfamily",
            Field::VariationsPostScriptPrefix => "Variations PS prefix",
            Field::Copyright => "Copyright",
            Field::Trademark => "Trademark",
            Field::Manufacturer => "Manufacturer",
            Field::Designer => "Designer",
            Field::Description => "Description",
            Field::LicenseDescription => "License description",
            Field::LicenseUrl => "License URL",
            Field::VendorUrl => "Vendor URL",
            Field::DesignerUrl => "Designer URL",
            Field::WeightClass => "Weight class",
            Field::WidthClass => "Width class",
            Field::BoldFlag => "Bold flag",
            Field::ItalicFlag => "Italic flag",
            Field::ObliqueFlag => "Oblique flag",
            Field::RegularFlag => "Regular flag",
        }
    }

    pub fn name_id(self) -> Option<u16> {
        match self {
            Field::Family => Some(name_id::FAMILY),
            Field::Subfamily => Some(name_id::SUBFAMILY),
            Field::UniqueId => Some(name_id::UNIQUE_ID),
            Field::FullName => Some(name_id::FULL_NAME),
            Field::Version => Some(name_id::VERSION),
            Field::PostScriptName => Some(name_id::POST_SCRIPT_NAME),
            Field::PreferredFamily => Some(name_id::TYPOGRAPHIC_FAMILY),
            Field::PreferredSubfamily => Some(name_id::TYPOGRAPHIC_SUBFAMILY),
            Field::CompatibleFullName => Some(18),
            Field::WwsFamily => Some(21),
            Field::WwsSubfamily => Some(22),
            Field::VariationsPostScriptPrefix => Some(25),
            Field::Copyright => Some(0),
            Field::Trademark => Some(7),
            Field::Manufacturer => Some(8),
            Field::Designer => Some(9),
            Field::Description => Some(10),
            Field::LicenseDescription => Some(13),
            Field::LicenseUrl => Some(14),
            Field::VendorUrl => Some(11),
            Field::DesignerUrl => Some(12),
            _ => None,
        }
    }
}

impl App {
    pub fn current_font(&self) -> Option<&FontFile> {
        self.fonts.get(self.selected_font)
    }

    pub fn handle_paste(&mut self, text: String) {
        let paths = parse_paths(&text);
        if !paths.is_empty() {
            self.load_paths(paths);
        }
    }

    pub fn handle_edit_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Esc => {
                self.editing = false;
                self.status = "Edit canceled".into();
            }
            KeyCode::Enter => {
                self.commit_edit();
                self.editing = false;
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
    }

    pub fn handle_normal_key(&mut self, key: KeyCode) {
        if self.pending_action.is_some() {
            match key {
                KeyCode::Left | KeyCode::Right | KeyCode::Tab => self.toggle_pending_choice(),
                KeyCode::Char('y') | KeyCode::Enter => self.confirm_pending_action(),
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.pending_action = None;
                    self.status = "Canceled".into();
                }
                _ => {}
            }

            return;
        }

        match key {
            KeyCode::Char('q') | KeyCode::Esc => {
                if matches!(self.screen, Screen::Help) {
                    self.screen = Screen::Editor;
                } else {
                    self.pending_action = Some(PendingAction::ConfirmQuit { selected_choice: 0 });
                }
            }
            KeyCode::Char('?') | KeyCode::Char('h') => {
                self.screen = Screen::Help;
            }
            KeyCode::Tab => self.toggle_focus(),
            KeyCode::Up => self.move_up(),
            KeyCode::Down => self.move_down(),
            KeyCode::Enter => {
                if matches!(self.focus, Focus::Fonts) {
                    self.toggle_focus();
                } else {
                    self.start_edit();
                }
            }
            KeyCode::Char('e') => self.start_edit(),
            KeyCode::Char('a') => self.request_apply_field_to_all(),
            KeyCode::Char('r') => match self.focus {
                Focus::Fonts => self.revert_current_font(),
                Focus::Fields => self.revert_selected_field(),
            },
            KeyCode::Char('s') => self.save_current_font(),
            KeyCode::Char('S') => self.save_all_fonts(),
            _ => {}
        }
    }

    pub fn from_args(paths: Vec<PathBuf>, output_dir: PathBuf) -> Self {
        let mut app = App::default();
        app.output_dir = output_dir;
        if !paths.is_empty() {
            app.load_paths(paths);
        }
        app
    }
    
    fn toggle_pending_choice(&mut self) {
        let choice = match &mut self.pending_action {
            Some(PendingAction::ApplyFieldToAll { selected_choice, .. }) => selected_choice,
            Some(PendingAction::ConfirmQuit { selected_choice }) => selected_choice,
            None => return,
        };
        *choice = if *choice == 0 { 1 } else { 0 };
    }
    
    fn request_apply_field_to_all(&mut self) {
        if self.fonts.is_empty() || !matches!(self.focus, Focus::Fields) {
            return;
        }
    
        let field = FIELDS[self.selected_field];
        let value = self.fonts[self.selected_font].edited.get(field);

        self.pending_action = Some(PendingAction::ApplyFieldToAll {
            field,
            value,
            selected_choice: 0,
        });
    
        self.status = "Confirm apply field".into();
    }

    fn confirm_pending_action(&mut self) {
        match self.pending_action.take() {
            Some(PendingAction::ApplyFieldToAll { selected_choice, .. }) => {
                if selected_choice == 1 {
                    self.apply_selected_field_to_all();
                } else {
                    self.status = "Canceled".into();
                }
            }
            Some(PendingAction::ConfirmQuit { selected_choice }) => {
                if selected_choice == 1 {
                    self.should_quit = true;
                } else {
                    self.status = "Canceled".into();
                }
            }
            None => {}
        }
    }

    fn apply_selected_field_to_all(&mut self) {
        let selected = self.selected_font;
        let field = FIELDS[self.selected_field];
        let value = self.fonts[selected].edited.get(field);

        for (i, font) in self.fonts.iter_mut().enumerate() {
            if i != selected {
                font.edited.set(field, value.clone());
            }
        }

        self.status = format!("Applied {} to all other fonts", field.label());
    }

    fn revert_current_font(&mut self) {
        if self.fonts.is_empty() {
            return;
        }
        let font = &mut self.fonts[self.selected_font];
        if !font.has_changes() {
            return;
        }
        font.edited = font.original.clone();
        self.status = "Reverted all changes".into();
    }

    fn revert_selected_field(&mut self) {
        if self.fonts.is_empty() || !matches!(self.focus, Focus::Fields) {
            return;
        }

        let field = FIELDS[self.selected_field];
        let original_value = self.fonts[self.selected_font].original.get(field);
        self.fonts[self.selected_font].edited.set(field, original_value);
        self.status = format!("Reverted {}", field.label());
    }

    fn load_paths(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            self.status = "No paths detected. Try dragging again or paste a path.".into();
            return;
        }

        let mut loaded = vec![];
        let mut first_error = None;

        for path in paths {
            match load_font(path.clone()) {
                Ok(font) => loaded.push(font),
                Err(e) if first_error.is_none() => first_error = Some(format!("{}: {}", path.display(), e)),
                Err(_) => {}
            }
        }

        if loaded.is_empty() {
            self.status = if let Some(err) = first_error {
                format!("Failed to load fonts: {}", err)
            } else {
                "No valid fonts loaded".into()
            };
            return;
        }

        let added = loaded.len();
        for font in loaded {
            if !self.fonts.iter().any(|f| f.path == font.path) {
                self.fonts.push(font);
            }
        }
        self.screen = Screen::Editor;
        self.status = if let Some(err) = first_error {
            format!("Added {} font(s). First error: {}", added, err)
        } else {
            format!("Added {} font(s) ({} total)", added, self.fonts.len())
        };
    }

    fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Fonts => Focus::Fields,
            Focus::Fields => Focus::Fonts,
        };
    }

    fn move_up(&mut self) {
        match self.focus {
            Focus::Fonts => self.selected_font = self.selected_font.saturating_sub(1),
            Focus::Fields => self.selected_field = self.selected_field.saturating_sub(1),
        }
    }

    fn move_down(&mut self) {
        match self.focus {
            Focus::Fonts => {
                if self.selected_font + 1 < self.fonts.len() {
                    self.selected_font += 1;
                }
            }
            Focus::Fields => {
                if self.selected_field + 1 < FIELDS.len() {
                    self.selected_field += 1;
                }
            }
        }
    }

    fn start_edit(&mut self) {
        if self.fonts.is_empty() || !matches!(self.focus, Focus::Fields) {
            return;
        }

        let field = FIELDS[self.selected_field];
        if !field.is_editable() {
            return;
        }

        self.input = self.fonts[self.selected_font].edited.get(field);
        self.editing = true;
        self.cursor_started = Instant::now();
        self.status = format!("Editing {}", field.label());
    }

    fn commit_edit(&mut self) {
        if self.fonts.is_empty() {
            return;
        }

        let field = FIELDS[self.selected_field];
        let value = self.input.clone();
        let old_value = self.fonts[self.selected_font].edited.get(field);
        self.fonts[self.selected_font].edited.set(field, value.clone());
        let new_value = self.fonts[self.selected_font].edited.get(field);
        self.status = if old_value == new_value && value != new_value {
            format!("Edited {} (invalid input, clamped to {})", field.label(), new_value)
        } else {
            format!("Edited {}", field.label())
        };
    }

    fn save_current_font(&mut self) {
        if self.fonts.is_empty() {
            return;
        }

        if let Err(err) = fs::create_dir_all(&self.output_dir) {
            self.status = format!("Could not create output folder: {err}");
            return;
        }

        let font = &self.fonts[self.selected_font];
        let name = font.path.file_name().and_then(|s| s.to_str()).unwrap_or("font").to_string();

        self.status = match save_fixed_font(font, &self.output_dir) {
            Ok(_) => format!("Saved {} to {}/", name, self.output_dir.display()),
            Err(e) => format!("Failed to save {}: {}", name, e),
        };
    }

    fn save_all_fonts(&mut self) {
        if self.fonts.is_empty() {
            return;
        }

        if let Err(err) = fs::create_dir_all(&self.output_dir) {
            self.status = format!("Could not create output folder: {err}");
            return;
        }

        let mut saved = 0;
        let mut first_error = None;

        for font in &self.fonts {
            match save_fixed_font(font, &self.output_dir) {
                Ok(_) => saved += 1,
                Err(e) if first_error.is_none() => {
                    first_error = Some(format!("{}: {}", font.path.display(), e));
                }
                Err(_) => {}
            }
        }

        self.status = if let Some(err) = first_error {
            format!("Saved {saved} font(s). First error: {}", err)
        } else {
            format!("Saved {} font(s) to {}/", saved, self.output_dir.display())
        };
    }
}

// Handles quoted paths, backslash escapes, and file:// URIs.
fn parse_paths(input: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = '\0';
    let mut escaped = false;

    for c in input.trim().chars() {
        if escaped {
            current.push(c);
            escaped = false;
            continue;
        }

        match c {
            '\\' => escaped = true,
            '"' | '\'' => {
                if in_quotes && c == quote_char {
                    in_quotes = false;
                    quote_char = '\0';
                } else if !in_quotes {
                    in_quotes = true;
                    quote_char = c;
                } else {
                    current.push(c);
                }
            }
            '\n' | '\r' | '\t' | ' ' if !in_quotes => {
                if !current.trim().is_empty() {
                    paths.push(clean_path(&current));
                    current.clear();
                }
            }
            _ => current.push(c),
        }
    }

    if !current.trim().is_empty() {
        paths.push(clean_path(&current));
    }

    paths
}

fn clean_path(raw: &str) -> PathBuf {
    let s = raw.trim();
    let s = s.strip_prefix("file://").unwrap_or(s);
    // file:///C:/path → strip leading slash before Windows drive letter
    let s = match s.strip_prefix('/') {
        Some(rest) if rest.starts_with(|c: char| c.is_ascii_alphabetic()) && rest[1..].starts_with(':') => rest,
        _ => s,
    };
    PathBuf::from(percent_decode(s))
}

fn percent_decode(s: &str) -> String {
    let mut buf = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = &s[i + 1..i + 3];
            if let Ok(value) = u8::from_str_radix(hex, 16) {
                buf.push(value);
                i += 3;
                continue;
            }
        }

        buf.push(bytes[i]);
        i += 1;
    }

    String::from_utf8_lossy(&buf).into_owned()
}
