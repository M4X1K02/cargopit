use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
};

use crate::config::{self, CargopitConfig, DeviceEntry, SimProfile};
use crate::consts;
use crate::diagnostics::{self, Diagnostics};
use crate::form::DeviceForm;
use crate::hardware::Discovery;
use crate::logs::LogState;
use crate::paths;
use crate::process::{self, ChildSession, ProcessStatus, SessionKind, TestScope};
use crate::schema::{DeviceClass, FieldId};
use crate::simapi_shm::{self, RunningGame, SimApiSession, TelemetryView};
use crate::simd_config::{self, SimdConfig, SimdSim};
use crate::templates;
use crate::tui_state::{self, PlayFlags, TuiState};
use crate::tyres::{self, TyreCar, TyreStore};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Dashboard,
    Devices,
    Settings,
    Telemetry,
    Logs,
    DeviceForm,
    DeviceTune,
    ProfileEdit,
    TemplatePicker,
    Confirm(ConfirmKind),
    SettingsSub(SettingsSub),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmKind {
    DeleteDevice,
    DeleteProfile,
    RestartAfterSave,
    ApplyTemplate(usize),
    DiscardUnsaved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSub {
    Flags,
    Simd,
    Lua,
    Tach,
    Tyres,
    Diagnostics,
    Raw,
}

pub struct App {
    pub screen: Screen,
    pub previous: Screen,
    pub tab: usize,
    pub config: CargopitConfig,
    pub config_path: PathBuf,
    pub raw_on_disk: String,
    pub tui_state: TuiState,
    pub status: ProcessStatus,
    pub discovery: Discovery,
    pub logs: LogState,
    pub message: String,
    pub last_tick: Instant,
    pub form: DeviceForm,
    pub device_index: usize,
    pub profile_index: usize,
    pub dashboard_index: usize,
    pub settings_index: usize,
    pub template_index: usize,
    pub simd: SimdConfig,
    pub simd_index: usize,
    pub simd_field: usize,
    pub tyres: TyreStore,
    pub tyre_index: usize,
    pub lua_index: usize,
    pub tach_max_revs: i64,
    pub tach_granularity: i64,
    pub tach_path: String,
    pub tach_field: usize,
    pub flags_field: usize,
    pub flags_original: PlayFlags,
    pub profile_edit: SimProfile,
    pub profile_original: SimProfile,
    pub profile_field: usize,
    pub tune_index: usize,
    pub should_quit: bool,
    pub telemetry: TelemetryView,
    pub telemetry_live: bool,
    pub flow_frame: u64,
    pub running_games: Vec<RunningGame>,
    shm: SimApiSession,
    prev_flow: Option<simapi_shm::FlowSnapshot>,
    extra_games: Vec<RunningGame>,
    last_flow: Option<Instant>,
    child_rx: Option<Receiver<(SessionKind, String)>>,
    child: Option<ChildSession>,
    diag: Diagnostics,
}

impl App {
    pub fn new() -> Result<Self> {
        paths::prepend_search_path();
        let tui_state = tui_state::load();
        let config_path = tui_state.config_path();
        let config = config::load_file(&config_path)?;
        let raw_on_disk = config::read_raw(&config_path).unwrap_or_default();
        let simd = simd_config::load(&paths::simd_config_path()).unwrap_or_default();
        let tyres = tyres::load(&paths::diameters_path()).unwrap_or_default();
        let flags_original = tui_state.play_flags.clone();
        let profile_index = tui_state.profile_index;
        let mut app = Self {
            screen: Screen::Dashboard,
            previous: Screen::Dashboard,
            tab: consts::TAB_DASHBOARD,
            config,
            config_path,
            raw_on_disk,
            tui_state,
            status: ProcessStatus::default(),
            discovery: Discovery::default(),
            logs: LogState::new(),
            message: String::new(),
            last_tick: Instant::now(),
            form: DeviceForm::blank(),
            device_index: 0,
            profile_index,
            dashboard_index: 0,
            settings_index: 0,
            template_index: 0,
            simd,
            simd_index: 0,
            simd_field: 0,
            tyres,
            tyre_index: 0,
            lua_index: 0,
            tach_max_revs: consts::DEFAULT_TACH_MAX_REVS,
            tach_granularity: consts::DEFAULT_GRANULARITY,
            tach_path: paths::config_home()
                .join(consts::CONFIG_DIR_NAME)
                .join("revburner.xml")
                .display()
                .to_string(),
            tach_field: 0,
            flags_field: 0,
            flags_original,
            profile_edit: SimProfile::default(),
            profile_original: SimProfile::default(),
            profile_field: 0,
            tune_index: 0,
            should_quit: false,
            telemetry: TelemetryView::default(),
            telemetry_live: false,
            flow_frame: 0,
            running_games: Vec::new(),
            shm: SimApiSession::new(),
            prev_flow: None,
            extra_games: Vec::new(),
            last_flow: None,
            child_rx: None,
            child: None,
            diag: Diagnostics::default(),
        };
        app.clamp_profile_index();
        app.refresh_runtime();
        app.sample_telemetry();
        Ok(app)
    }

    pub fn current_profile(&self) -> Option<&SimProfile> {
        self.config.profiles.get(self.profile_index)
    }

    pub fn current_profile_mut(&mut self) -> Option<&mut SimProfile> {
        self.config.profiles.get_mut(self.profile_index)
    }

    pub fn profile_display_name(&self) -> String {
        let Some(profile) = self.current_profile() else {
            return consts::TITLE_NO_PROFILES.to_string();
        };
        if !profile.name.is_empty() {
            return truncate_profile_name(&profile.name);
        }
        format!("{} {}", consts::TITLE_PROFILE, self.profile_index + 1)
    }

    pub fn profile_pin_label(&self) -> String {
        let count = self.config.profiles.len().max(1);
        format!(
            "{} {}/{} {}",
            consts::LABEL_PROFILE,
            self.profile_index + 1,
            count,
            self.profile_display_name()
        )
    }

    fn clamp_profile_index(&mut self) {
        if self.config.profiles.is_empty() {
            self.profile_index = 0;
            return;
        }
        self.profile_index = self.profile_index.min(self.config.profiles.len() - 1);
    }

    fn persist_profile_selection(&mut self) {
        self.tui_state.profile_index = self.profile_index;
        if let Err(err) = tui_state::save(&self.tui_state) {
            self.message = err.to_string();
        }
    }

    pub fn current_device(&self) -> Option<&DeviceEntry> {
        self.current_profile()?.devices.get(self.device_index)
    }

    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diag
    }

    fn device_presence_keys(&self) -> Vec<(DeviceClass, String, String)> {
        self.current_profile()
            .map(|profile| {
                profile
                    .devices
                    .iter()
                    .map(|device| {
                        (
                            device.class(),
                            device.get_str(consts::KEY_DEVID).unwrap_or("").to_string(),
                            device
                                .get_str(consts::KEY_DEVPATH)
                                .unwrap_or("")
                                .to_string(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn refresh_diagnostics(&mut self) {
        let devices = self.device_presence_keys();
        let mut diag = diagnostics::collect(&self.discovery, &devices);
        diag.simapi_exists = self.shm.mapped();
        diag.simapi_live = self.telemetry_live;
        self.diag = diag;
    }

    fn refresh_presence(&mut self) {
        let devices = self.device_presence_keys();
        diagnostics::apply_presence(&mut self.diag, &self.discovery, &devices);
    }

    fn sync_simapi_diag(&mut self) {
        self.diag.simapi_exists = self.shm.mapped();
        self.diag.simapi_live = self.telemetry_live;
    }

    fn refresh_runtime(&mut self) {
        let listing = process::process_listing();
        self.status = process::check_processes_from(&listing);
        self.discovery = Discovery::live();
        self.logs.refresh_files();
        self.extra_games = simapi_shm::extras_from_listing(&self.simd, &listing);
        self.refresh_diagnostics();
        if !self.status.cleaned_pid_files.is_empty() {
            self.message = format!(
                "Cleaned stale PID files: {}",
                self.status.cleaned_pid_files.join(", ")
            );
        }
        self.last_tick = Instant::now();
    }

    pub fn simapi_exists(&self) -> bool {
        self.shm.mapped()
    }

    pub fn test_running(&self) -> bool {
        self.child
            .as_ref()
            .is_some_and(|session| session.kind == SessionKind::Test)
    }

    pub fn tick(&mut self) {
        let shm_event = self.shm.drain();
        let status_due =
            self.last_tick.elapsed() >= Duration::from_millis(consts::STATUS_REFRESH_MS);
        if shm_event {
            if !self.shm.mapped() {
                self.prev_flow = None;
                self.last_flow = None;
            }
            if !status_due {
                self.refresh_extra_games();
            }
        }
        if status_due {
            self.refresh_runtime();
        }
        self.sample_telemetry();
        self.sync_simapi_diag();
        self.drain_child();
        if self.child.is_some() {
            self.logs.refresh_files();
        }
    }

    fn refresh_extra_games(&mut self) {
        self.extra_games = simapi_shm::extras_from_listing(&self.simd, &process::process_listing());
    }

    fn sample_telemetry(&mut self) {
        let Some(view) = self.shm.sample() else {
            self.telemetry = TelemetryView::default();
            self.telemetry_live = self.test_running();
            self.prev_flow = None;
            if self.telemetry_live {
                self.last_flow = Some(Instant::now());
                self.advance_flow_frame();
            }
            self.running_games =
                simapi_shm::games_with_flow(&self.extra_games, self.telemetry_live);
            self.ensure_test_pipeline_node();
            self.bind_settings_to_live_game();
            return;
        };
        let changed = simapi_shm::telemetry_sending(&view, self.prev_flow);
        self.prev_flow = Some(simapi_shm::flow_snapshot(&view));
        if changed || self.test_running() {
            self.last_flow = Some(Instant::now());
        }
        self.telemetry_live = self.test_running()
            || changed
            || simapi_shm::flow_hold_active(self.last_flow, Instant::now());
        if self.telemetry_live {
            self.advance_flow_frame();
        }
        self.telemetry = view;
        self.running_games =
            simapi_shm::resolve_games(&self.simd, &view, self.telemetry_live, &self.extra_games);
        self.ensure_test_pipeline_node();
        self.bind_settings_to_live_game();
    }

    pub fn settings_game_label(&self) -> String {
        let id = self.tui_state.settings_game_id;
        if id == consts::SETTINGS_GAME_IDLE {
            return consts::SETTINGS_BOUND_IDLE.to_string();
        }
        if let Some(name) = self.simd.name_for_game_id(id) {
            return name.to_string();
        }
        if let Some(game) = self.running_games.iter().find(|game| game.game_id == id) {
            return game.name.clone();
        }
        format!("{} {id}", consts::LABEL_SIM_ID)
    }

    pub(crate) fn bind_settings_to_live_game(&mut self) {
        let live_id = self.live_settings_game_id();
        if live_id == self.tui_state.settings_game_id {
            return;
        }
        self.tui_state
            .store_flags_for(self.tui_state.settings_game_id);
        self.tui_state.settings_game_id = live_id;
        self.tui_state.play_flags = self.tui_state.flags_for(live_id);
        self.flags_original = self.tui_state.play_flags.clone();
        let Some(index) = self.simd.index_for_game_id(live_id) else {
            return;
        };
        self.simd_index = index;
    }

    fn live_settings_game_id(&self) -> u64 {
        let mut fallback = consts::SETTINGS_GAME_IDLE;
        for game in &self.running_games {
            if !is_settings_game(game) {
                continue;
            }
            if game.sending {
                return game.game_id;
            }
            if fallback == consts::SETTINGS_GAME_IDLE {
                fallback = game.game_id;
            }
        }
        fallback
    }

    fn advance_flow_frame(&mut self) {
        self.flow_frame = self.flow_frame.wrapping_add(1);
    }

    pub fn pipeline_pit(&self) -> bool {
        self.status.cargopit_running || self.test_running()
    }

    fn ensure_test_pipeline_node(&mut self) {
        if !self.test_running() {
            return;
        }
        if let Some(game) = self
            .running_games
            .iter_mut()
            .find(|game| game.name == consts::LABEL_TEST)
        {
            game.sending = true;
            game.status_label = consts::LABEL_TELEMETRY_LIVE;
            return;
        }
        self.running_games.insert(
            0,
            RunningGame {
                name: consts::LABEL_TEST.to_string(),
                game_id: 0,
                sending: true,
                status_label: consts::LABEL_TELEMETRY_LIVE,
            },
        );
    }

    fn drain_child(&mut self) {
        if let Some(rx) = &self.child_rx {
            loop {
                match rx.try_recv() {
                    Ok((kind, line)) => self.logs.push(kind.as_str().to_string(), line),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.child_rx = None;
                        break;
                    }
                }
            }
        }
        let Some(session) = self.child.as_mut() else {
            return;
        };
        match session.child.try_wait() {
            Ok(Some(status)) => {
                self.message = process::session_exit_message(session.kind, status);
                self.child = None;
            }
            Ok(None) => {}
            Err(err) => {
                self.message = format!("wait failed: {err}");
                self.child = None;
            }
        }
    }

    pub fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => {
                self.handle_mouse(mouse);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }
        if self.form.edit_buffer.is_some()
            && matches!(self.screen, Screen::DeviceForm | Screen::DeviceTune)
        {
            self.handle_form_edit(key.code);
            return Ok(());
        }
        if is_global_tab(key.code) && is_main_tab(&self.screen) {
            self.switch_tab_key(key.code);
            return Ok(());
        }
        if is_profile_cycle_key(key.code) && is_main_tab(&self.screen) {
            let delta = if key.code == consts::KEY_PREV_PROFILE {
                -1
            } else {
                1
            };
            self.cycle_profile(delta);
            return Ok(());
        }
        match self.screen {
            Screen::Dashboard => self.handle_dashboard(key.code),
            Screen::Devices => self.handle_devices(key.code),
            Screen::Settings => self.handle_settings(key.code),
            Screen::Telemetry => self.handle_telemetry(key.code),
            Screen::Logs => self.handle_logs(key.code),
            Screen::DeviceForm => self.handle_form(key.code)?,
            Screen::DeviceTune => self.handle_tune(key.code)?,
            Screen::ProfileEdit => self.handle_profile_edit(key.code)?,
            Screen::TemplatePicker => self.handle_templates(key.code),
            Screen::Confirm(ref kind) => self.handle_confirm(key.code, kind.clone())?,
            Screen::SettingsSub(sub) => self.handle_settings_sub(key.code, sub)?,
        }
        Ok(())
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        if !matches!(self.screen, Screen::SettingsSub(SettingsSub::Simd)) {
            return;
        }
        let delta = match mouse.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollLeft => -1,
            MouseEventKind::ScrollDown | MouseEventKind::ScrollRight => 1,
            _ => return,
        };
        if simd_wheel_pans_columns(mouse) {
            self.simd_field = step_saturating(self.simd_field, self.simd_column_count(), delta);
            return;
        }
        self.simd_index = step_saturating(self.simd_index, self.simd.sims.len(), delta);
    }

    fn switch_tab_key(&mut self, code: KeyCode) {
        let tab = match code {
            consts::KEY_TAB => (self.tab + 1) % consts::TAB_COUNT,
            consts::KEY_BACK_TAB => (self.tab + consts::TAB_COUNT - 1) % consts::TAB_COUNT,
            consts::KEY_START => consts::TAB_DASHBOARD,
            consts::KEY_TAB2 => consts::TAB_DEVICES,
            consts::KEY_TAB3 => consts::TAB_SETTINGS,
            consts::KEY_TAB4 => consts::TAB_TELEMETRY,
            consts::KEY_TAB5 => consts::TAB_LOGS,
            _ => return,
        };
        self.set_tab(tab);
    }

    fn set_tab(&mut self, tab: usize) {
        self.tab = tab;
        self.screen = match tab {
            consts::TAB_DEVICES => Screen::Devices,
            consts::TAB_SETTINGS => Screen::Settings,
            consts::TAB_TELEMETRY => Screen::Telemetry,
            consts::TAB_LOGS => Screen::Logs,
            _ => Screen::Dashboard,
        };
    }

    fn handle_dashboard(&mut self, code: KeyCode) {
        match code {
            consts::KEY_QUIT | consts::KEY_QUIT_UPPER | consts::KEY_ESC => self.should_quit = true,
            consts::KEY_UP | consts::KEY_K => {
                self.dashboard_index =
                    wrap_index(self.dashboard_index, consts::DASHBOARD_ACTIONS.len(), -1);
            }
            consts::KEY_DOWN | consts::KEY_J => {
                self.dashboard_index =
                    wrap_index(self.dashboard_index, consts::DASHBOARD_ACTIONS.len(), 1);
            }
            consts::KEY_ENTER => self.run_dashboard_action(self.dashboard_index),
            consts::KEY_TEST => self.toggle_test(),
            _ => {}
        }
    }

    fn handle_telemetry(&mut self, code: KeyCode) {
        match code {
            consts::KEY_QUIT | consts::KEY_QUIT_UPPER | consts::KEY_ESC => self.should_quit = true,
            _ => {}
        }
    }

    fn run_dashboard_action(&mut self, index: usize) {
        match consts::DASHBOARD_ACTIONS.get(index).copied() {
            Some(consts::ACTION_START) => self.start_play(),
            Some(consts::ACTION_TEST) => self.toggle_test(),
            Some(consts::ACTION_RESTART) => self.restart_play(),
            Some(consts::ACTION_STOP) => self.stop_all(),
            _ => {}
        }
    }

    fn start_play(&mut self) {
        self.status = process::check_processes();
        if self.play_is_live() {
            self.message = consts::MSG_PLAY_ALREADY_RUNNING.into();
            return;
        }
        self.spawn_play_session();
    }

    fn play_is_live(&self) -> bool {
        self.child
            .as_ref()
            .is_some_and(|session| session.kind == SessionKind::Play)
            || self.status.cargopit_running
    }

    fn spawn_play_session(&mut self) {
        let scope = TestScope {
            config_index: Some(self.profile_index),
            device_index: None,
        };
        match process::spawn_session_with_scope(
            SessionKind::Play,
            &self.tui_state.play_flags,
            &self.config_path,
            scope,
        ) {
            Ok(session) => {
                self.attach_child(session);
                self.message = consts::MSG_STARTED_PLAY.into();
                self.set_tab(consts::TAB_LOGS);
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn restart_play(&mut self) {
        self.kill_tracked_child();
        let _ = process::stop_play();
        process::wait_until_stopped(consts::BINARY_CARGOPIT);
        self.status.cargopit_running = false;
        self.spawn_play_session();
    }

    fn toggle_test(&mut self) {
        self.request_test(TestScope::default(), true);
    }

    fn start_editor_test(&mut self) {
        if self.test_running() {
            self.stop_tracked_test();
            return;
        }
        if !self.commit_form(false) {
            return;
        }
        self.start_device_test(false);
    }

    fn start_device_test(&mut self, switch_to_logs: bool) {
        let scope = TestScope {
            config_index: Some(self.profile_index),
            device_index: Some(self.device_index),
        };
        self.request_test(scope, switch_to_logs);
    }

    fn request_test(&mut self, scope: TestScope, switch_to_logs: bool) {
        if self.test_running() {
            self.stop_tracked_test();
            return;
        }
        self.launch_test(scope, switch_to_logs);
    }

    fn launch_test(&mut self, scope: TestScope, switch_to_logs: bool) {
        self.status = process::check_processes();
        self.kill_tracked_child();
        if self.status.cargopit_running {
            let _ = process::stop_play();
            self.status = process::check_processes();
        }
        match process::spawn_session_with_scope(
            SessionKind::Test,
            &self.tui_state.play_flags,
            &self.config_path,
            scope,
        ) {
            Ok(session) => {
                self.attach_child(session);
                self.message = if scope.device_index.is_some() {
                    consts::MSG_STARTED_DEVICE_TEST.into()
                } else {
                    consts::MSG_STARTED_TEST.into()
                };
                if switch_to_logs {
                    self.set_tab(consts::TAB_LOGS);
                }
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn attach_child(&mut self, mut session: ChildSession) {
        let (tx, rx) = mpsc::channel();
        spawn_pipe_reader(session.child.stdout.take(), session.kind, tx.clone());
        spawn_pipe_reader(session.child.stderr.take(), session.kind, tx);
        self.child_rx = Some(rx);
        self.child = Some(session);
    }

    fn stop_all(&mut self) {
        self.kill_tracked_child();
        let stopped = process::stop_all();
        self.status = process::check_processes();
        self.message = if stopped.is_empty() {
            consts::MSG_NO_SERVICES.into()
        } else {
            format!("Stopped: {}", stopped.join(", "))
        };
    }

    fn kill_tracked_child(&mut self) {
        if let Some(mut session) = self.child.take() {
            process::kill_session(&mut session);
        }
        self.child_rx = None;
    }

    fn stop_tracked_test(&mut self) {
        if !self.test_running() {
            self.message = consts::MSG_NO_TEST_RUNNING.into();
            return;
        }
        self.kill_tracked_child();
        self.message = consts::MSG_STOPPED_TEST.into();
    }

    fn handle_devices(&mut self, code: KeyCode) {
        match code {
            consts::KEY_QUIT | consts::KEY_QUIT_UPPER => self.should_quit = true,
            consts::KEY_UP | consts::KEY_K => self.move_device(-1),
            consts::KEY_DOWN | consts::KEY_J => self.move_device(1),
            consts::KEY_G_UPPER => self.reorder_device(1),
            consts::KEY_K_UPPER => self.reorder_device(-1),
            consts::KEY_SPACE => self.toggle_enabled(),
            consts::KEY_TEST => self.start_device_test(true),
            consts::KEY_ADD => self.open_new_form(),
            consts::KEY_EDIT => self.open_edit_form(),
            consts::KEY_DUPLICATE => self.duplicate_device(),
            consts::KEY_DELETE => self.open_confirm(ConfirmKind::DeleteDevice),
            consts::KEY_DELETE_UPPER => self.open_confirm(ConfirmKind::DeleteProfile),
            consts::KEY_TEMPLATE => {
                self.previous = Screen::Devices;
                self.screen = Screen::TemplatePicker;
            }
            consts::KEY_ENTER => self.open_tune(),
            consts::KEY_SAVE => {
                self.previous = Screen::Devices;
                self.profile_edit = self.current_profile().cloned().unwrap_or_default();
                self.profile_original = self.profile_edit.clone();
                self.profile_field = consts::PROFILE_FIELD_NAME;
                self.screen = Screen::ProfileEdit;
            }
            _ => {}
        }
    }

    fn cycle_profile(&mut self, delta: isize) {
        if self.config.profiles.is_empty() {
            return;
        }
        let len = self.config.profiles.len() as isize;
        self.profile_index = (self.profile_index as isize + delta).rem_euclid(len) as usize;
        self.device_index = 0;
        self.persist_profile_selection();
        self.refresh_presence();
    }

    fn move_device(&mut self, delta: isize) {
        let Some(profile) = self.current_profile() else {
            return;
        };
        if profile.devices.is_empty() {
            return;
        }
        self.device_index = wrap_index(self.device_index, profile.devices.len(), delta);
    }

    fn reorder_device(&mut self, delta: isize) {
        let len = match self.current_profile() {
            Some(profile) if !profile.devices.is_empty() => profile.devices.len(),
            _ => return,
        };
        let current = self.device_index;
        let next = (current as isize + delta).rem_euclid(len as isize) as usize;
        if let Some(profile) = self.current_profile_mut() {
            profile.devices.swap(current, next);
        }
        self.device_index = next;
        self.persist_config();
    }

    fn toggle_enabled(&mut self) {
        let index = self.device_index;
        let Some(profile) = self.current_profile_mut() else {
            return;
        };
        let Some(device) = profile.devices.get_mut(index) else {
            return;
        };
        let enabled = !device.enabled();
        device.set_bool(consts::KEY_ENABLED, enabled);
        self.persist_config();
    }

    fn open_new_form(&mut self) {
        self.form = DeviceForm::blank();
        self.previous = Screen::Devices;
        self.screen = Screen::DeviceForm;
    }

    fn open_edit_form(&mut self) {
        let Some(device) = self.current_device().cloned() else {
            self.message = "No device to edit".into();
            return;
        };
        self.form = DeviceForm::new(device, false);
        self.previous = Screen::Devices;
        self.screen = Screen::DeviceForm;
    }

    fn duplicate_device(&mut self) {
        let Some(device) = self.current_device().cloned() else {
            return;
        };
        let insert_at = self.device_index + 1;
        if let Some(profile) = self.current_profile_mut() {
            profile.devices.insert(insert_at, device);
        }
        self.device_index = insert_at;
        self.persist_config();
    }

    fn open_tune(&mut self) {
        let Some(device) = self.current_device().cloned() else {
            return;
        };
        self.form = DeviceForm::new(device, false);
        self.tune_index = 0;
        self.previous = Screen::Devices;
        self.screen = Screen::DeviceTune;
    }

    fn handle_form(&mut self, code: KeyCode) -> Result<()> {
        match code {
            consts::KEY_ESC => self.leave_or_confirm_discard(),
            consts::KEY_UP | consts::KEY_K => self.form.move_field(-1),
            consts::KEY_DOWN | consts::KEY_J => self.form.move_field(1),
            consts::KEY_LEFT | consts::KEY_H | consts::KEY_MINUS => {
                self.form.cycle_current(&self.discovery, -1);
            }
            consts::KEY_RIGHT | consts::KEY_L | consts::KEY_PLUS | consts::KEY_EQUALS => {
                self.form.cycle_current(&self.discovery, 1);
            }
            consts::KEY_ENTER => self.form.begin_edit(),
            consts::KEY_SAVE => self.save_form()?,
            consts::KEY_BACKSPACE => self.form.clear_current(),
            consts::KEY_TEST => self.start_editor_test(),
            _ => {}
        }
        Ok(())
    }

    fn handle_form_edit(&mut self, code: KeyCode) {
        let Some(buffer) = self.form.edit_buffer.as_mut() else {
            return;
        };
        match code {
            consts::KEY_ESC => self.form.cancel_edit(),
            consts::KEY_ENTER => self.form.commit_edit(),
            consts::KEY_BACKSPACE => {
                buffer.pop();
            }
            KeyCode::Char(ch) => buffer.push(ch),
            _ => {}
        }
    }

    fn save_form(&mut self) -> Result<()> {
        let _ = self.commit_form(true);
        Ok(())
    }

    fn commit_form(&mut self, leave: bool) -> bool {
        if let Err(err) = self.form.validate() {
            self.form.error = Some(err.clone());
            self.message = err;
            return false;
        }
        let device = self.form.device.clone();
        let is_new = self.form.is_new;
        let index = self.device_index;
        let Some(profile) = self.current_profile_mut() else {
            return false;
        };
        if is_new {
            profile.devices.push(device);
            self.device_index = profile.devices.len().saturating_sub(1);
        } else if let Some(slot) = profile.devices.get_mut(index) {
            *slot = device;
        }
        self.persist_config();
        self.form.mark_saved();
        if !leave {
            return true;
        }
        self.screen = Screen::Devices;
        if self.play_is_live() {
            self.open_confirm(ConfirmKind::RestartAfterSave);
        }
        true
    }

    fn handle_tune(&mut self, code: KeyCode) -> Result<()> {
        let fields = tune_fields(&self.form);
        match code {
            consts::KEY_ESC => self.leave_or_confirm_discard(),
            consts::KEY_UP | consts::KEY_K => {
                self.tune_index = wrap_index(self.tune_index, fields.len().max(1), -1);
            }
            consts::KEY_DOWN | consts::KEY_J => {
                self.tune_index = wrap_index(self.tune_index, fields.len().max(1), 1);
            }
            consts::KEY_LEFT | consts::KEY_H | consts::KEY_MINUS => {
                if let Some(field) = fields.get(self.tune_index).copied() {
                    self.form.field_index = index_of_field(&self.form, field);
                    self.form.nudge(field, -1, true);
                }
            }
            consts::KEY_RIGHT | consts::KEY_L | consts::KEY_PLUS | consts::KEY_EQUALS => {
                if let Some(field) = fields.get(self.tune_index).copied() {
                    self.form.field_index = index_of_field(&self.form, field);
                    self.form.nudge(field, 1, true);
                }
            }
            consts::KEY_BACKSPACE => {
                if let Some(field) = fields.get(self.tune_index).copied() {
                    self.form.field_index = index_of_field(&self.form, field);
                    self.form.clear_current();
                }
            }
            consts::KEY_SAVE => self.save_form()?,
            consts::KEY_TEST => self.start_editor_test(),
            consts::KEY_APPLY => {
                self.save_form()?;
                if self.form.error.is_some() {
                    return Ok(());
                }
                if self.play_is_live() {
                    self.restart_play();
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_profile_edit(&mut self, code: KeyCode) -> Result<()> {
        match code {
            consts::KEY_ESC => self.leave_or_confirm_discard(),
            consts::KEY_ADD => {
                let mut profile = SimProfile::default();
                profile.name = format!(
                    "{} {}",
                    consts::TITLE_PROFILE,
                    self.config.profiles.len() + 1
                );
                self.config.profiles.push(profile);
                self.profile_index = self.config.profiles.len() - 1;
                self.persist_config();
                self.persist_profile_selection();
                self.screen = Screen::Devices;
            }
            consts::KEY_DUPLICATE => {
                if let Some(profile) = self.current_profile().cloned() {
                    self.config.profiles.push(profile);
                    self.profile_index = self.config.profiles.len() - 1;
                    self.persist_config();
                    self.persist_profile_selection();
                }
                self.screen = Screen::Devices;
            }
            consts::KEY_SAVE | consts::KEY_ENTER => {
                if let Some(slot) = self.config.profiles.get_mut(self.profile_index) {
                    slot.name = self.profile_edit.name.clone();
                    slot.sim = self.profile_edit.sim.clone();
                    slot.car = self.profile_edit.car.clone();
                    slot.api = self.profile_edit.api.clone();
                }
                self.persist_config();
                self.screen = Screen::Devices;
            }
            KeyCode::Char(ch) => {
                if self.profile_field == consts::PROFILE_FIELD_NAME {
                    self.profile_edit.name.push(ch);
                }
            }
            consts::KEY_BACKSPACE => {
                if self.profile_field == consts::PROFILE_FIELD_NAME {
                    self.profile_edit.name.pop();
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_templates(&mut self, code: KeyCode) {
        let count = templates::all().len();
        match code {
            consts::KEY_ESC => self.screen = Screen::Devices,
            consts::KEY_UP | consts::KEY_K => {
                self.template_index = wrap_index(self.template_index, count, -1);
            }
            consts::KEY_DOWN | consts::KEY_J => {
                self.template_index = wrap_index(self.template_index, count, 1);
            }
            consts::KEY_ENTER => {
                self.open_confirm(ConfirmKind::ApplyTemplate(self.template_index));
            }
            _ => {}
        }
    }

    fn handle_confirm(&mut self, code: KeyCode, kind: ConfirmKind) -> Result<()> {
        match code {
            consts::KEY_CONFIRM_YES => {
                let previous = self.previous.clone();
                self.apply_confirm(kind.clone())?;
                self.screen = match kind {
                    ConfirmKind::DiscardUnsaved => editor_parent(&previous),
                    _ => Screen::Devices,
                };
            }
            consts::KEY_CONFIRM_NO | consts::KEY_ESC => {
                self.screen = self.previous.clone();
            }
            _ => {}
        }
        Ok(())
    }

    fn apply_confirm(&mut self, kind: ConfirmKind) -> Result<()> {
        match kind {
            ConfirmKind::DeleteDevice => {
                let index = self.device_index;
                if let Some(profile) = self.current_profile_mut() {
                    if index < profile.devices.len() {
                        profile.devices.remove(index);
                    }
                }
                self.device_index = index.saturating_sub(1);
                self.persist_config();
            }
            ConfirmKind::DeleteProfile => {
                if self.config.profiles.len() > 1 {
                    self.config.profiles.remove(self.profile_index);
                    self.profile_index = self.profile_index.saturating_sub(1);
                    self.persist_config();
                    self.persist_profile_selection();
                } else {
                    self.message = "Cannot delete the last profile".into();
                }
            }
            ConfirmKind::RestartAfterSave => {
                self.restart_play();
            }
            ConfirmKind::ApplyTemplate(index) => {
                if let Some(template) = templates::all().get(index) {
                    let devices = (template.devices)();
                    if let Some(profile) = self.current_profile_mut() {
                        profile.devices.extend(devices);
                    }
                    self.persist_config();
                    self.message = format!("Inserted template {}", template.name);
                }
            }
            ConfirmKind::DiscardUnsaved => {
                if matches!(self.previous, Screen::SettingsSub(SettingsSub::Flags)) {
                    self.tui_state.play_flags = self.flags_original.clone();
                }
            }
        }
        Ok(())
    }

    fn handle_settings(&mut self, code: KeyCode) {
        match code {
            consts::KEY_QUIT | consts::KEY_QUIT_UPPER => self.should_quit = true,
            consts::KEY_UP | consts::KEY_K => {
                self.settings_index =
                    wrap_index(self.settings_index, consts::SETTINGS_ITEMS.len(), -1);
            }
            consts::KEY_DOWN | consts::KEY_J => {
                self.settings_index =
                    wrap_index(self.settings_index, consts::SETTINGS_ITEMS.len(), 1);
            }
            consts::KEY_ENTER => {
                let sub = match self.settings_index {
                    consts::SETTINGS_INDEX_FLAGS => SettingsSub::Flags,
                    consts::SETTINGS_INDEX_SIMD => SettingsSub::Simd,
                    consts::SETTINGS_INDEX_LUA => SettingsSub::Lua,
                    consts::SETTINGS_INDEX_TACH => SettingsSub::Tach,
                    consts::SETTINGS_INDEX_TYRES => SettingsSub::Tyres,
                    consts::SETTINGS_INDEX_DIAGNOSTICS => SettingsSub::Diagnostics,
                    _ => SettingsSub::Raw,
                };
                if sub == SettingsSub::Flags {
                    self.flags_original = self.tui_state.play_flags.clone();
                    self.flags_field = 0;
                }
                self.previous = Screen::Settings;
                self.screen = Screen::SettingsSub(sub);
            }
            _ => {}
        }
    }

    fn handle_settings_sub(&mut self, code: KeyCode, sub: SettingsSub) -> Result<()> {
        if code == consts::KEY_ESC {
            self.leave_or_confirm_discard();
            return Ok(());
        }
        match sub {
            SettingsSub::Flags => self.handle_flags(code),
            SettingsSub::Simd => self.handle_simd(code)?,
            SettingsSub::Lua => self.handle_lua(code)?,
            SettingsSub::Tach => self.handle_tach(code)?,
            SettingsSub::Tyres => self.handle_tyres(code)?,
            SettingsSub::Diagnostics | SettingsSub::Raw => {}
        }
        Ok(())
    }

    fn handle_flags(&mut self, code: KeyCode) {
        match code {
            consts::KEY_UP | consts::KEY_K => {
                self.flags_field = wrap_index(self.flags_field, consts::FLAG_FIELD_COUNT, -1);
            }
            consts::KEY_DOWN | consts::KEY_J => {
                self.flags_field = wrap_index(self.flags_field, consts::FLAG_FIELD_COUNT, 1);
            }
            consts::KEY_LEFT | consts::KEY_H => self.nudge_flag(-1),
            consts::KEY_RIGHT | consts::KEY_L | consts::KEY_ENTER => self.nudge_flag(1),
            consts::KEY_BACKSPACE => self.clear_flag(),
            consts::KEY_SAVE => {
                self.tui_state
                    .store_flags_for(self.tui_state.settings_game_id);
                match tui_state::save(&self.tui_state) {
                    Ok(()) => {
                        self.flags_original = self.tui_state.play_flags.clone();
                        self.message = "Play flags saved".into();
                    }
                    Err(err) => self.message = err.to_string(),
                }
            }
            _ => {}
        }
    }

    fn nudge_flag(&mut self, delta: i32) {
        match self.flags_field {
            consts::FLAG_FIELD_VERBOSITY => {
                let current = i32::from(self.tui_state.play_flags.verbosity);
                let count = i32::from(consts::VERBOSITY_LEVEL_COUNT);
                self.tui_state.play_flags.verbosity = (current + delta).rem_euclid(count) as u8;
            }
            consts::FLAG_FIELD_DISABLE_AUDIO => {
                self.tui_state.play_flags.disable_audio = !self.tui_state.play_flags.disable_audio;
            }
            consts::FLAG_FIELD_UDP => {
                self.tui_state.play_flags.udp = !self.tui_state.play_flags.udp;
            }
            consts::FLAG_FIELD_FPS => self.cycle_fps(delta),
            _ => {}
        }
    }

    fn cycle_fps(&mut self, delta: i32) {
        let current = self.tui_state.play_flags.fps;
        let choices = consts::FPS_FLAG_CHOICES;
        let index = choices
            .iter()
            .position(|item| *item == current)
            .unwrap_or(0);
        let next = (index as i32 + delta).rem_euclid(choices.len() as i32) as usize;
        self.tui_state.play_flags.fps = choices[next];
    }

    fn clear_flag(&mut self) {
        match self.flags_field {
            consts::FLAG_FIELD_FPS => self.tui_state.play_flags.fps = None,
            consts::FLAG_FIELD_LOG => self.tui_state.play_flags.log_file = None,
            _ => {}
        }
    }

    fn handle_simd(&mut self, code: KeyCode) -> Result<()> {
        match code {
            consts::KEY_ADD => {
                if self.simd.sims.is_empty() {
                    self.simd = simd_config::stub_from_bundled();
                    self.simd_index = 0;
                } else {
                    self.simd.sims.push(SimdSim {
                        settings: Vec::new(),
                    });
                    self.simd_index = self.simd.sims.len() - 1;
                }
                self.simd_field = 0;
                simd_config::save(&paths::simd_config_path(), &self.simd)?;
            }
            consts::KEY_DELETE => {
                if self.simd_index < self.simd.sims.len() {
                    self.simd.sims.remove(self.simd_index);
                    simd_config::save(&paths::simd_config_path(), &self.simd)?;
                    self.clamp_simd_selection();
                }
            }
            consts::KEY_UP | consts::KEY_K => {
                self.simd_index = step_saturating(self.simd_index, self.simd.sims.len(), -1);
            }
            consts::KEY_DOWN | consts::KEY_J => {
                self.simd_index = step_saturating(self.simd_index, self.simd.sims.len(), 1);
            }
            consts::KEY_LEFT | consts::KEY_H => {
                self.simd_field = step_saturating(self.simd_field, self.simd_column_count(), -1);
            }
            consts::KEY_RIGHT | consts::KEY_L => {
                self.simd_field = step_saturating(self.simd_field, self.simd_column_count(), 1);
            }
            consts::KEY_PAGE_UP => {
                let delta = -(consts::SIMD_PAGE_ROWS as i32);
                self.simd_index = step_saturating(self.simd_index, self.simd.sims.len(), delta);
            }
            consts::KEY_PAGE_DOWN => {
                let delta = consts::SIMD_PAGE_ROWS as i32;
                self.simd_index = step_saturating(self.simd_index, self.simd.sims.len(), delta);
            }
            consts::KEY_HOME => self.simd_index = 0,
            consts::KEY_END => {
                self.simd_index = self.simd.sims.len().saturating_sub(1);
            }
            consts::KEY_ENTER | consts::KEY_PLUS | consts::KEY_EQUALS => {
                self.cycle_simd_telemetry(1)?;
            }
            consts::KEY_MINUS => self.cycle_simd_telemetry(-1)?,
            consts::KEY_SAVE => simd_config::save(&paths::simd_config_path(), &self.simd)?,
            _ => {}
        }
        Ok(())
    }

    fn simd_column_count(&self) -> usize {
        simd_config::table_columns(&self.simd).len().max(1)
    }

    fn cycle_simd_telemetry(&mut self, delta: i32) -> Result<()> {
        let columns = simd_config::table_columns(&self.simd);
        let Some(key) = columns.get(self.simd_field).map(String::as_str) else {
            return Ok(());
        };
        if key != consts::SIMD_FIELD_TELEMETRY {
            return Ok(());
        }
        if self.simd_index >= self.simd.sims.len() {
            return Ok(());
        }
        let source = self.simd.sims[self.simd_index].cycle_telemetry_source(delta);
        simd_config::save(&paths::simd_config_path(), &self.simd)?;
        self.message = format!(
            "{} telemetry: {source}",
            self.simd.sims[self.simd_index].display_name()
        );
        Ok(())
    }

    fn clamp_simd_selection(&mut self) {
        if self.simd.sims.is_empty() {
            self.simd_index = 0;
            self.simd_field = 0;
            return;
        }
        if self.simd_index >= self.simd.sims.len() {
            self.simd_index = self.simd.sims.len() - 1;
        }
        let columns = self.simd_column_count();
        if self.simd_field >= columns {
            self.simd_field = columns - 1;
        }
    }

    fn handle_lua(&mut self, code: KeyCode) -> Result<()> {
        let count = consts::BUNDLED_LUA.len().max(1);
        match code {
            consts::KEY_UP | consts::KEY_K => {
                self.lua_index = wrap_index(self.lua_index, count, -1);
            }
            consts::KEY_DOWN | consts::KEY_J => {
                self.lua_index = wrap_index(self.lua_index, count, 1);
            }
            consts::KEY_ENTER => {
                if let Some(name) = consts::BUNDLED_LUA.get(self.lua_index) {
                    match diagnostics::copy_lua_template(name) {
                        Ok(path) => self.message = format!("Copied {}", path.display()),
                        Err(err) => self.message = err.to_string(),
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_tach(&mut self, code: KeyCode) -> Result<()> {
        match code {
            consts::KEY_UP | consts::KEY_K => {
                self.tach_field = wrap_index(self.tach_field, consts::TACH_FIELD_COUNT, -1);
            }
            consts::KEY_DOWN | consts::KEY_J => {
                self.tach_field = wrap_index(self.tach_field, consts::TACH_FIELD_COUNT, 1);
            }
            consts::KEY_LEFT | consts::KEY_H => self.nudge_tach(-1),
            consts::KEY_RIGHT | consts::KEY_L => self.nudge_tach(1),
            consts::KEY_ENTER | consts::KEY_SAVE => {
                match process::spawn_tachometer(
                    self.tach_max_revs,
                    self.tach_granularity,
                    &self.tach_path,
                ) {
                    Ok(out) => self.message = out,
                    Err(err) => self.message = err.to_string(),
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn nudge_tach(&mut self, delta: i64) {
        match self.tach_field {
            consts::TACH_FIELD_MAX_REVS => {
                self.tach_max_revs = (self.tach_max_revs + delta * consts::TACH_REVS_STEP)
                    .max(consts::TACH_REVS_MIN);
            }
            consts::TACH_FIELD_GRANULARITY => {
                let current = self.tach_granularity;
                let labels = consts::GRANULARITY_ALLOWED;
                let index = labels.iter().position(|v| *v == current).unwrap_or(0);
                let next = (index as i64 + delta).rem_euclid(labels.len() as i64) as usize;
                self.tach_granularity = labels[next];
            }
            _ => {}
        }
    }

    fn handle_tyres(&mut self, code: KeyCode) -> Result<()> {
        match code {
            consts::KEY_ADD => {
                self.tyres.cars.push(TyreCar::default());
                tyres::save(&paths::diameters_path(), &self.tyres)?;
            }
            consts::KEY_DELETE => {
                if self.tyre_index < self.tyres.cars.len() {
                    self.tyres.cars.remove(self.tyre_index);
                    tyres::save(&paths::diameters_path(), &self.tyres)?;
                }
            }
            consts::KEY_UP | consts::KEY_K => {
                self.tyre_index = wrap_index(self.tyre_index, self.tyres.cars.len().max(1), -1);
            }
            consts::KEY_DOWN | consts::KEY_J => {
                self.tyre_index = wrap_index(self.tyre_index, self.tyres.cars.len().max(1), 1);
            }
            consts::KEY_SAVE => tyres::save(&paths::diameters_path(), &self.tyres)?,
            _ => {}
        }
        Ok(())
    }

    fn handle_logs(&mut self, code: KeyCode) {
        match code {
            consts::KEY_QUIT | consts::KEY_QUIT_UPPER => self.should_quit = true,
            consts::KEY_UP | consts::KEY_K => self.logs.scroll = self.logs.scroll.saturating_add(1),
            consts::KEY_DOWN | consts::KEY_J => {
                self.logs.scroll = self.logs.scroll.saturating_sub(1)
            }
            consts::KEY_TEST => {
                if self.test_running() {
                    self.stop_tracked_test();
                    return;
                }
                self.logs.filter = self.logs.filter.cycle();
            }
            consts::KEY_SPACE => self.logs.filter = self.logs.filter.cycle(),
            _ => {}
        }
    }

    fn open_confirm(&mut self, kind: ConfirmKind) {
        self.previous = self.screen.clone();
        self.screen = Screen::Confirm(kind);
    }

    fn leave_or_confirm_discard(&mut self) {
        if !self.current_editor_dirty() {
            self.screen = editor_parent(&self.screen);
            return;
        }
        self.open_confirm(ConfirmKind::DiscardUnsaved);
    }

    fn current_editor_dirty(&self) -> bool {
        match self.screen {
            Screen::DeviceForm | Screen::DeviceTune => self.form.is_dirty(),
            Screen::ProfileEdit => self.profile_edit.name != self.profile_original.name,
            Screen::SettingsSub(SettingsSub::Flags) => {
                self.tui_state.play_flags != self.flags_original
            }
            _ => false,
        }
    }

    fn persist_config(&mut self) {
        self.raw_on_disk = config::read_raw(&self.config_path).unwrap_or_default();
        match config::save_file(&self.config_path, &self.config) {
            Ok(()) => self.message = format!("Saved {}", self.config_path.display()),
            Err(err) => self.message = err.to_string(),
        }
        self.refresh_presence();
    }
}

fn spawn_pipe_reader<R: std::io::Read + Send + 'static>(
    pipe: Option<R>,
    kind: SessionKind,
    tx: mpsc::Sender<(SessionKind, String)>,
) {
    let Some(pipe) = pipe else {
        return;
    };
    thread::spawn(move || {
        let reader = BufReader::new(pipe);
        for line in reader.lines().flatten() {
            if tx.send((kind, line)).is_err() {
                break;
            }
        }
    });
}

fn is_settings_game(game: &RunningGame) -> bool {
    game.game_id != consts::SETTINGS_GAME_IDLE && game.name != consts::LABEL_TEST
}

fn wrap_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    (current as isize + delta).rem_euclid(len as isize) as usize
}

fn step_saturating(current: usize, len: usize, delta: i32) -> usize {
    if len == 0 {
        return 0;
    }
    let last = (len - 1) as i32;
    (current as i32 + delta).clamp(0, last) as usize
}

fn simd_wheel_pans_columns(mouse: MouseEvent) -> bool {
    matches!(
        mouse.kind,
        MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight
    ) || !mouse.modifiers.contains(KeyModifiers::SHIFT)
}

fn is_profile_cycle_key(code: KeyCode) -> bool {
    matches!(code, consts::KEY_PREV_PROFILE | consts::KEY_NEXT_PROFILE)
}

fn truncate_profile_name(name: &str) -> String {
    let max = consts::PROFILE_PIN_NAME_MAX;
    if name.chars().count() <= max {
        return name.to_string();
    }
    name.chars().take(max).collect()
}

fn is_global_tab(code: KeyCode) -> bool {
    matches!(
        code,
        consts::KEY_TAB
            | consts::KEY_BACK_TAB
            | consts::KEY_START
            | consts::KEY_TAB2
            | consts::KEY_TAB3
            | consts::KEY_TAB4
            | consts::KEY_TAB5
    )
}

fn is_main_tab(screen: &Screen) -> bool {
    matches!(
        screen,
        Screen::Dashboard | Screen::Devices | Screen::Settings | Screen::Telemetry | Screen::Logs
    )
}

fn editor_parent(screen: &Screen) -> Screen {
    match screen {
        Screen::DeviceForm | Screen::DeviceTune | Screen::ProfileEdit => Screen::Devices,
        Screen::SettingsSub(_) => Screen::Settings,
        other => other.clone(),
    }
}

pub fn tune_fields(form: &DeviceForm) -> Vec<FieldId> {
    form.fields()
        .into_iter()
        .filter(|field| {
            matches!(
                field,
                FieldId::Volume
                    | FieldId::Pan
                    | FieldId::Frequency
                    | FieldId::FrequencyMax
                    | FieldId::Amplitude
                    | FieldId::AmplitudeMax
                    | FieldId::Threshold
                    | FieldId::Duration
                    | FieldId::Ampfactor
                    | FieldId::Fanpower
                    | FieldId::Fps
            )
        })
        .collect()
}

fn index_of_field(form: &DeviceForm, field: FieldId) -> usize {
    form.fields()
        .iter()
        .position(|item| *item == field)
        .unwrap_or(0)
}
