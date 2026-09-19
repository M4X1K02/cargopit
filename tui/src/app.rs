use crate::config::{self, DeviceEntry, MonocoqueFile};
use crate::consts::*;
use crate::form::DeviceForm;
use crate::logs;
use crate::paths;
use crate::process::{self, ManagedChild, ProcessStatus};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

pub enum Screen {
    Main,
    DeviceEdit(DeviceForm),
    DeviceTune { device_index: usize },
    ConfirmDelete { device_index: usize },
}

pub struct App {
    pub should_quit: bool,
    pub tab: usize,
    pub screen: Screen,
    pub file: MonocoqueFile,
    pub config_path: PathBuf,
    pub selected_config: usize,
    pub selected_device: usize,
    pub dashboard_action: usize,
    pub status: ProcessStatus,
    pub message: Option<String>,
    pub log_lines: Vec<String>,
    pub log_offset: usize,
    pub play_child: Option<ManagedChild>,
    pub test_child: Option<ManagedChild>,
    log_tx: Sender<String>,
    log_rx: Receiver<String>,
}

impl App {
    pub fn new() -> Result<Self> {
        let config_path = paths::config_path();
        let file = config::load_from_path(&config_path)?;
        let selected_config = file.default_config_index();
        let (log_tx, log_rx) = process::log_channel();
        let mut app = Self {
            should_quit: false,
            tab: TAB_DASHBOARD,
            screen: Screen::Main,
            file,
            config_path,
            selected_config,
            selected_device: 0,
            dashboard_action: ACTION_START,
            status: process::refresh_status(),
            message: None,
            log_lines: logs::read_recent_logs(),
            log_offset: 0,
            play_child: None,
            test_child: None,
            log_tx,
            log_rx,
        };
        app.clamp_device_selection();
        Ok(app)
    }

    pub fn tick(&mut self) {
        self.status = process::refresh_status();
        if !self.status.notes.is_empty() {
            self.message = Some(self.status.notes.join("; "));
        }
        while let Ok(line) = self.log_rx.try_recv() {
            self.push_log(line);
        }
        if let Some(child) = self.play_child.as_mut() {
            if let Some(note) = child.try_reap() {
                self.push_log(note.clone());
                self.message = Some(note);
                self.play_child = None;
            }
        }
        if let Some(child) = self.test_child.as_mut() {
            if let Some(note) = child.try_reap() {
                self.push_log(note.clone());
                self.message = Some(note);
                self.test_child = None;
            }
        }
        if self.tab == TAB_LOGS {
            if self.log_offset == 0 {
                self.log_lines = merge_logs(&self.log_lines, logs::read_recent_logs());
            }
        }
    }

    fn push_log(&mut self, line: String) {
        self.log_lines.push(line);
        if self.log_lines.len() > MAX_LOG_LINES {
            let extra = self.log_lines.len() - MAX_LOG_LINES;
            self.log_lines.drain(0..extra);
        }
    }

    pub fn current_devices(&self) -> &[DeviceEntry] {
        self.file
            .configs
            .get(self.selected_config)
            .map(|config| config.devices.as_slice())
            .unwrap_or(&[])
    }

    fn current_devices_mut(&mut self) -> Option<&mut Vec<DeviceEntry>> {
        self.file
            .configs
            .get_mut(self.selected_config)
            .map(|config| &mut config.devices)
    }

    fn clamp_device_selection(&mut self) {
        let count = self.current_devices().len();
        if count == 0 {
            self.selected_device = 0;
            return;
        }
        if self.selected_device >= count {
            self.selected_device = count - 1;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        match &self.screen {
            Screen::Main => self.handle_main_key(key),
            Screen::DeviceEdit(_) => self.handle_edit_key(key),
            Screen::DeviceTune { .. } => self.handle_tune_key(key),
            Screen::ConfirmDelete { .. } => self.handle_confirm_key(key),
        }
    }

    fn handle_main_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(KEY_QUIT) | KeyCode::Esc => self.should_quit = true,
            KeyCode::Tab => self.tab = (self.tab + 1) % TAB_COUNT,
            KeyCode::BackTab => self.tab = (self.tab + TAB_COUNT - 1) % TAB_COUNT,
            KeyCode::Char(KEY_TAB_DASHBOARD) => self.tab = TAB_DASHBOARD,
            KeyCode::Char(KEY_TAB_DEVICES) => self.tab = TAB_DEVICES,
            KeyCode::Char(KEY_TAB_LOGS) => self.tab = TAB_LOGS,
            _ => match self.tab {
                TAB_DASHBOARD => self.handle_dashboard_key(key),
                TAB_DEVICES => self.handle_devices_key(key),
                TAB_LOGS => self.handle_logs_key(key),
                _ => {}
            },
        }
    }

    fn handle_dashboard_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char(KEY_UP) => {
                self.dashboard_action =
                    (self.dashboard_action + DASHBOARD_ACTION_COUNT - 1) % DASHBOARD_ACTION_COUNT;
            }
            KeyCode::Down | KeyCode::Char(KEY_DOWN) => {
                self.dashboard_action = (self.dashboard_action + 1) % DASHBOARD_ACTION_COUNT;
            }
            KeyCode::Enter => self.run_dashboard_action(self.dashboard_action),
            _ => {}
        }
    }

    fn run_dashboard_action(&mut self, action: usize) {
        match action {
            ACTION_START => self.start_play(),
            ACTION_TEST => self.start_test(),
            ACTION_RESTART => {
                self.stop_all();
                self.start_play();
            }
            ACTION_STOP => self.stop_all(),
            _ => {}
        }
    }

    fn start_play(&mut self) {
        if self.status.monocoque_running {
            self.message = Some("monocoque is already running".to_string());
            return;
        }
        match process::spawn_play(self.log_tx.clone()) {
            Ok(child) => {
                self.message = Some("Started monocoque play".to_string());
                self.push_log("[tui] started monocoque play".to_string());
                self.play_child = Some(child);
            }
            Err(error) => self.message = Some(error.to_string()),
        }
    }

    fn start_test(&mut self) {
        match process::spawn_test(self.log_tx.clone()) {
            Ok(child) => {
                self.message = Some("Started monocoque test".to_string());
                self.push_log("[tui] started monocoque test".to_string());
                self.test_child = Some(child);
                self.tab = TAB_LOGS;
            }
            Err(error) => self.message = Some(error.to_string()),
        }
    }

    fn stop_all(&mut self) {
        let stopped = process::stop_all_services();
        self.play_child = None;
        self.test_child = None;
        if stopped.is_empty() {
            self.message = Some("No running services found to stop".to_string());
        } else {
            self.message = Some(format!("Stopped: {}", stopped.join(", ")));
        }
    }

    fn handle_devices_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char(KEY_UP) => self.move_device(-1),
            KeyCode::Down | KeyCode::Char(KEY_DOWN) => self.move_device(1),
            KeyCode::Left | KeyCode::Char('[') => self.move_config(-1),
            KeyCode::Right | KeyCode::Char(']') => self.move_config(1),
            KeyCode::Char(KEY_ADD) => self.open_add(),
            KeyCode::Char(KEY_EDIT) => self.open_edit(),
            KeyCode::Char(KEY_DELETE) => self.open_delete(),
            KeyCode::Enter => self.open_tune(),
            _ => {}
        }
    }

    fn move_device(&mut self, delta: i32) {
        let count = self.current_devices().len() as i32;
        if count == 0 {
            return;
        }
        self.selected_device = (self.selected_device as i32 + delta).rem_euclid(count) as usize;
    }

    fn move_config(&mut self, delta: i32) {
        let count = self.file.configs.len() as i32;
        if count == 0 {
            return;
        }
        self.selected_config = (self.selected_config as i32 + delta).rem_euclid(count) as usize;
        self.selected_device = 0;
    }

    fn open_add(&mut self) {
        let form = DeviceForm::new(config::new_device(CLASS_SOUND), self.selected_config, None);
        self.screen = Screen::DeviceEdit(form);
    }

    fn open_edit(&mut self) {
        let Some(device) = self.current_devices().get(self.selected_device).cloned() else {
            self.message = Some("No device selected".to_string());
            return;
        };
        let form = DeviceForm::new(device, self.selected_config, Some(self.selected_device));
        self.screen = Screen::DeviceEdit(form);
    }

    fn open_tune(&mut self) {
        if self.current_devices().is_empty() {
            self.message = Some("No device selected".to_string());
            return;
        }
        self.screen = Screen::DeviceTune {
            device_index: self.selected_device,
        };
    }

    fn open_delete(&mut self) {
        if self.current_devices().is_empty() {
            self.message = Some("No device selected".to_string());
            return;
        }
        self.screen = Screen::ConfirmDelete {
            device_index: self.selected_device,
        };
    }

    fn handle_edit_key(&mut self, key: KeyEvent) {
        let mut save = false;
        let mut test = false;
        let mut close = false;
        {
            let Screen::DeviceEdit(form) = &mut self.screen else {
                return;
            };
            if form.text_editing {
                match key.code {
                    KeyCode::Esc => form.text_editing = false,
                    KeyCode::Enter => form.commit_text_edit(),
                    KeyCode::Backspace => {
                        form.text_buffer.pop();
                    }
                    KeyCode::Char(ch) => form.text_buffer.push(ch),
                    _ => {}
                }
                return;
            }
            match key.code {
                KeyCode::Esc | KeyCode::Backspace => close = true,
                KeyCode::Up | KeyCode::Char(KEY_UP) => form.move_field(-1),
                KeyCode::Down | KeyCode::Char(KEY_DOWN) => form.move_field(1),
                KeyCode::Left | KeyCode::Char(KEY_LEFT) => form.cycle(-1),
                KeyCode::Right | KeyCode::Char(KEY_RIGHT) => form.cycle(1),
                KeyCode::Enter => form.begin_text_edit(),
                KeyCode::Char(KEY_SAVE) => save = true,
                KeyCode::Char(KEY_TEST) => test = true,
                _ => {}
            }
        }
        if close {
            self.screen = Screen::Main;
        }
        if save {
            self.save_form();
        }
        if test {
            self.start_test();
        }
    }

    fn save_form(&mut self) {
        let Screen::DeviceEdit(form) = &self.screen else {
            return;
        };
        let config_index = form.config_index;
        let device = form.device.clone();
        let device_index = form.device_index;
        let Some(config) = self.file.configs.get_mut(config_index) else {
            self.message = Some("Configuration entry is missing".to_string());
            return;
        };
        if let Some(index) = device_index {
            if index < config.devices.len() {
                config.devices[index] = device;
            }
        } else {
            config.devices.push(device);
            self.selected_device = config.devices.len().saturating_sub(1);
        }
        match config::save_to_path(&self.config_path, &self.file) {
            Ok(()) => {
                self.message = Some(format!("Saved {}", self.config_path.display()));
                self.screen = Screen::Main;
                self.tab = TAB_DEVICES;
            }
            Err(error) => self.message = Some(error.to_string()),
        }
    }

    fn handle_tune_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char(KEY_QUIT) => {
                self.screen = Screen::Main;
                self.tab = TAB_DEVICES;
            }
            _ => {}
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(KEY_CONFIRM_YES) => self.delete_selected(),
            KeyCode::Char(KEY_CONFIRM_NO) | KeyCode::Esc => self.screen = Screen::Main,
            _ => {}
        }
    }

    fn delete_selected(&mut self) {
        let Screen::ConfirmDelete { device_index } = self.screen else {
            return;
        };
        if let Some(devices) = self.current_devices_mut() {
            if device_index < devices.len() {
                devices.remove(device_index);
            }
        }
        match config::save_to_path(&self.config_path, &self.file) {
            Ok(()) => self.message = Some("Device deleted".to_string()),
            Err(error) => self.message = Some(error.to_string()),
        }
        self.screen = Screen::Main;
        self.clamp_device_selection();
    }

    fn handle_logs_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char(KEY_UP) => {
                self.log_offset = self.log_offset.saturating_add(1);
            }
            KeyCode::Down | KeyCode::Char(KEY_DOWN) => {
                self.log_offset = self.log_offset.saturating_sub(1);
            }
            KeyCode::Char('G') => self.log_offset = 0,
            _ => {}
        }
    }
}

fn merge_logs(runtime: &[String], files: Vec<String>) -> Vec<String> {
    let mut combined = files;
    for line in runtime {
        if line.starts_with('[') && !combined.iter().any(|existing| existing == line) {
            combined.push(line.clone());
        }
    }
    if combined.len() > MAX_LOG_LINES {
        combined = combined[combined.len() - MAX_LOG_LINES..].to_vec();
    }
    combined
}
