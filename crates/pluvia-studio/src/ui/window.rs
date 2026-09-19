use crate::ipc::IpcClient;
use crate::scanner::{scan_installed_skins, DiscoveredSkin};
use crate::ui::installer_dialog::open_package_installer;
use gtk4::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use libadwaita::prelude::*;
use pluvia_core::bangs::write_key_value_to_file;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

pub struct StudioWindow {
    window: adw::ApplicationWindow,
    ipc: Rc<IpcClient>,
    active_list: gtk::ListBox,
    library_box: gtk::Box,
    detail_box: gtk::Box,
    status_label: gtk::Label,
    daemon_banner: adw::Banner,
    search_entry: gtk::SearchEntry,
    search_query: Rc<RefCell<String>>,
    skins: Rc<RefCell<BTreeMap<String, Vec<DiscoveredSkin>>>>,
    selected_skin: Rc<RefCell<Option<DiscoveredSkin>>>,
}

impl StudioWindow {
    pub fn build(app: &adw::Application) -> Self {
        let ipc = Rc::new(IpcClient::new());
        let skins = Rc::new(RefCell::new(scan_installed_skins()));
        let selected_skin = Rc::new(RefCell::new(None));
        let search_query = Rc::new(RefCell::new(String::new()));

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Pluvia Studio")
            .default_width(1100)
            .default_height(750)
            .build();

        // Main layout containing the banner at top and the paned split below
        let main_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

        // 1-Click Daemon Start Banner
        let daemon_banner = adw::Banner::new("Pluvia Desktop Engine is stopped. Widgets cannot render.");
        daemon_banner.set_button_label(Some("Start Engine"));
        daemon_banner.set_use_markup(true);
        main_box.append(&daemon_banner);

        // Main horizontal split
        let paned = gtk::Paned::new(gtk::Orientation::Horizontal);
        paned.set_position(380);
        paned.set_vexpand(true);

        // --- Left Sidebar ---
        let sidebar_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sidebar_box.set_size_request(340, -1);

        let sidebar_header = adw::HeaderBar::new();
        let title = adw::WindowTitle::new("Pluvia Studio", "");
        sidebar_header.set_title_widget(Some(&title));

        let add_btn = gtk::Button::builder()
            .tooltip_text("Install .rmskin Package")
            .icon_name("list-add-symbolic")
            .build();

        let power_btn = gtk::Button::builder()
            .tooltip_text("Start / Stop Desktop Engine")
            .icon_name("system-run-symbolic")
            .build();

        let refresh_btn = gtk::Button::builder()
            .tooltip_text("Refresh All Skins")
            .icon_name("view-refresh-symbolic")
            .build();

        sidebar_header.pack_start(&add_btn);
        sidebar_header.pack_start(&power_btn);
        sidebar_header.pack_end(&refresh_btn);
        sidebar_box.append(&sidebar_header);

        // Search Entry for fast navigation
        let search_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        search_box.set_margin_start(12);
        search_box.set_margin_end(12);
        search_box.set_margin_top(8);
        search_box.set_margin_bottom(4);

        let search_entry = gtk::SearchEntry::new();
        search_entry.set_placeholder_text(Some("Search widgets and suites..."));
        search_entry.set_hexpand(true);
        search_box.append(&search_entry);
        sidebar_box.append(&search_box);

        // Daemon status bar
        let status_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        status_box.set_margin_start(16);
        status_box.set_margin_end(16);
        status_box.set_margin_top(4);
        status_box.set_margin_bottom(8);

        let status_label = gtk::Label::new(Some("Checking engine status..."));
        status_label.set_hexpand(true);
        status_label.set_xalign(0.0);
        status_box.append(&status_label);
        sidebar_box.append(&status_box);

        let sidebar_scroll = gtk::ScrolledWindow::new();
        sidebar_scroll.set_vexpand(true);

        let lists_box = gtk::Box::new(gtk::Orientation::Vertical, 16);
        lists_box.set_margin_start(12);
        lists_box.set_margin_end(12);
        lists_box.set_margin_top(4);
        lists_box.set_margin_bottom(16);

        // Active skins group
        let active_group = adw::PreferencesGroup::new();
        active_group.set_title("Active on Desktop");
        let active_list = gtk::ListBox::new();
        active_list.add_css_class("boxed-list");
        active_group.add(&active_list);
        lists_box.append(&active_group);

        // Installed library container (contains collapsible suite expanders)
        let library_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
        lists_box.append(&library_box);

        sidebar_scroll.set_child(Some(&lists_box));
        sidebar_box.append(&sidebar_scroll);
        paned.set_start_child(Some(&sidebar_box));

        // --- Right Detail View ---
        let detail_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        detail_box.set_hexpand(true);

        let detail_header = adw::HeaderBar::new();
        let detail_title = adw::WindowTitle::new("Skin Inspector", "Select a skin to customize");
        detail_header.set_title_widget(Some(&detail_title));
        detail_box.append(&detail_header);

        let placeholder_label = gtk::Label::new(Some("Select a widget from the library on the left to configure."));
        placeholder_label.set_vexpand(true);
        placeholder_label.add_css_class("dim-label");
        detail_box.append(&placeholder_label);

        paned.set_end_child(Some(&detail_box));
        main_box.append(&paned);
        window.set_content(Some(&main_box));

        let studio = Self {
            window,
            ipc,
            active_list,
            library_box,
            detail_box,
            status_label,
            daemon_banner,
            search_entry,
            search_query,
            skins,
            selected_skin,
        };

        studio.setup_events(add_btn, power_btn, refresh_btn);
        studio.refresh_ui();
        studio
    }

    pub fn present(&self) {
        self.window.present();
    }

    fn setup_events(&self, add_btn: gtk::Button, power_btn: gtk::Button, refresh_btn: gtk::Button) {
        let win_clone = self.window.clone();
        let studio_refresh = self.clone_refresh_helper();
        add_btn.connect_clicked(move |_| {
            let refresh = studio_refresh.clone();
            open_package_installer(&win_clone, move || {
                refresh();
            });
        });

        // 1-Click Banner Action to Start Engine
        let ipc_banner = self.ipc.clone();
        let studio_refresh_banner = self.clone_refresh_helper();
        self.daemon_banner.connect_button_clicked(move |_| {
            let _ = ipc_banner.start_daemon();
            studio_refresh_banner();
        });

        // Power button in sidebar header (1-click start/restart)
        let ipc_power = self.ipc.clone();
        let studio_refresh_power = self.clone_refresh_helper();
        power_btn.connect_clicked(move |_| {
            if ipc_power.is_daemon_running() {
                let _ = ipc_power.stop_daemon();
            } else {
                let _ = ipc_power.start_daemon();
            }
            studio_refresh_power();
        });

        let studio_refresh_all = self.clone_refresh_helper();
        let ipc_clone = self.ipc.clone();
        refresh_btn.connect_clicked(move |_| {
            let _ = ipc_clone.refresh_all();
            studio_refresh_all();
        });

        // Live Search Filtering
        let search_query = self.search_query.clone();
        let studio_refresh_search = self.clone_refresh_helper();
        self.search_entry.connect_search_changed(move |entry| {
            *search_query.borrow_mut() = entry.text().to_string();
            studio_refresh_search();
        });
    }

    fn clone_refresh_helper(&self) -> Rc<dyn Fn()> {
        let studio_weak = self.downgrade();
        Rc::new(move || {
            if let Some(studio) = studio_weak.upgrade() {
                studio.refresh_ui();
            }
        })
    }

    fn downgrade(&self) -> StudioWindowWeak {
        StudioWindowWeak {
            window: self.window.clone(),
            ipc: self.ipc.clone(),
            active_list: self.active_list.clone(),
            library_box: self.library_box.clone(),
            detail_box: self.detail_box.clone(),
            status_label: self.status_label.clone(),
            daemon_banner: self.daemon_banner.clone(),
            search_entry: self.search_entry.clone(),
            search_query: self.search_query.clone(),
            skins: self.skins.clone(),
            selected_skin: self.selected_skin.clone(),
        }
    }

    pub fn refresh_ui(&self) {
        let daemon_running = self.ipc.is_daemon_running();

        // Update 1-click Banner and Status Label
        if daemon_running {
            self.daemon_banner.set_revealed(false);
            if let Ok(state) = self.ipc.get_state() {
                self.status_label.set_text(&format!(
                    "🟢 Engine Active (Backend: {}, Active: {})",
                    state.backend, state.active_skins.len()
                ));
                self.status_label.remove_css_class("error");
                self.status_label.add_css_class("success");
            }
        } else {
            self.daemon_banner.set_revealed(true);
            self.status_label.set_text("⭕ Engine Stopped (Click 'Start Engine' above)");
            self.status_label.remove_css_class("success");
            self.status_label.add_css_class("error");
        }

        // Refresh active skins list
        while let Some(child) = self.active_list.first_child() {
            self.active_list.remove(&child);
        }

        let mut active_skin_paths: Vec<String> = Vec::new();

        if let Ok(active_skins) = self.ipc.list_skins() {
            if active_skins.is_empty() {
                let row = adw::ActionRow::new();
                row.set_title("No widgets active on desktop");
                row.set_subtitle("Click 'Load' on any widget below to display");
                self.active_list.append(&row);
            } else {
                for skin in active_skins {
                    active_skin_paths.push(skin.path.clone());
                    let row = adw::ActionRow::new();
                    row.set_title(&skin.id);
                    row.set_subtitle(&format!(
                        "{}x{} at ({}, {})",
                        skin.bounds.width, skin.bounds.height, skin.bounds.x, skin.bounds.y
                    ));

                    let unload_btn = gtk::Button::builder()
                        .label("Unload")
                        .valign(gtk::Align::Center)
                        .css_classes(["destructive-action", "flat"])
                        .build();

                    let ipc = self.ipc.clone();
                    let skin_id = skin.id.clone();
                    let refresh_helper = self.clone_refresh_helper();
                    unload_btn.connect_clicked(move |_| {
                        let _ = ipc.unload_skin(&skin_id);
                        refresh_helper();
                    });

                    row.add_suffix(&unload_btn);
                    self.active_list.append(&row);
                }
            }
        } else {
            let row = adw::ActionRow::new();
            row.set_title("Engine disconnected");
            row.set_subtitle("Start the engine to view active widgets");
            self.active_list.append(&row);
        }

        // Refresh library list with Collapsible Suite Groups and Search Filtering
        *self.skins.borrow_mut() = scan_installed_skins();
        while let Some(child) = self.library_box.first_child() {
            self.library_box.remove(&child);
        }

        let query = self.search_query.borrow().to_lowercase();
        let skins_map = self.skins.borrow();

        if skins_map.is_empty() {
            let group = adw::PreferencesGroup::new();
            let row = adw::ActionRow::new();
            row.set_title("No skins found in ~/.config/pluvia/skins");
            group.add(&row);
            self.library_box.append(&group);
        } else {
            for (suite, skin_items) in skins_map.iter() {
                // Filter items by search query
                let filtered_items: Vec<&DiscoveredSkin> = skin_items
                    .iter()
                    .filter(|s| {
                        if query.is_empty() {
                            true
                        } else {
                            s.name.to_lowercase().contains(&query)
                                || suite.to_lowercase().contains(&query)
                                || s.path.to_string_lossy().to_lowercase().contains(&query)
                        }
                    })
                    .collect();

                if filtered_items.is_empty() {
                    continue;
                }

                let expander = adw::ExpanderRow::new();
                expander.set_title(suite);
                expander.set_subtitle(&format!("{} widget(s)", filtered_items.len()));
                expander.set_icon_name(Some("folder-symbolic"));
                expander.set_expanded(true);

                let suite_group = adw::PreferencesGroup::new();

                for skin in filtered_items {
                    let row = adw::ActionRow::new();
                    row.set_title(&skin.name);
                    row.set_subtitle(&skin.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
                    row.set_activatable(true);

                    let skin_path_str = skin.path.to_string_lossy().to_string();
                    let is_active = active_skin_paths.iter().any(|ap| ap == &skin_path_str || skin_path_str.ends_with(ap));

                    if is_active {
                        let active_badge = gtk::Label::builder()
                            .label("Active")
                            .css_classes(["success", "caption"])
                            .valign(gtk::Align::Center)
                            .margin_end(8)
                            .build();
                        row.add_prefix(&active_badge);
                    }

                    // 1-Click Load / Unload Button right in the list row!
                    let action_btn = if is_active {
                        gtk::Button::builder()
                            .label("Unload")
                            .valign(gtk::Align::Center)
                            .css_classes(["destructive-action", "flat"])
                            .build()
                    } else {
                        gtk::Button::builder()
                            .label("Load")
                            .valign(gtk::Align::Center)
                            .css_classes(["suggested-action", "flat"])
                            .build()
                    };

                    let ipc = self.ipc.clone();
                    let path_clone = skin.path.clone();
                    let refresh_helper = self.clone_refresh_helper();
                    action_btn.connect_clicked(move |_| {
                        if is_active {
                            let _ = ipc.unload_skin(&path_clone.to_string_lossy());
                        } else {
                            let _ = ipc.load_skin(&path_clone);
                        }
                        refresh_helper();
                    });

                    row.add_suffix(&action_btn);

                    let skin_clone = skin.clone();
                    let studio_weak = self.downgrade();
                    row.connect_activated(move |_| {
                        if let Some(studio) = studio_weak.upgrade() {
                            studio.show_skin_inspector(&skin_clone);
                        }
                    });

                    expander.add_row(&row);
                }

                suite_group.add(&expander);
                self.library_box.append(&suite_group);
            }

            if self.selected_skin.borrow().is_none() {
                if let Some(first_skin) = skins_map.values().next().and_then(|v| v.first()) {
                    self.show_skin_inspector(first_skin);
                }
            }
        }
    }

    pub fn show_skin_inspector(&self, skin: &DiscoveredSkin) {
        *self.selected_skin.borrow_mut() = Some(skin.clone());

        // Clear existing details
        while let Some(child) = self.detail_box.first_child() {
            self.detail_box.remove(&child);
        }

        // Header
        let header = adw::HeaderBar::new();
        let title = adw::WindowTitle::new(&skin.name, &skin.suite);
        header.set_title_widget(Some(&title));
        self.detail_box.append(&header);

        let scroll = gtk::ScrolledWindow::new();
        scroll.set_vexpand(true);

        let content_box = gtk::Box::new(gtk::Orientation::Vertical, 16);
        content_box.set_margin_start(24);
        content_box.set_margin_end(24);
        content_box.set_margin_top(16);
        content_box.set_margin_bottom(24);

        // Action banner
        let banner_group = adw::PreferencesGroup::new();
        let action_row = adw::ActionRow::new();
        action_row.set_title(&skin.name);
        action_row.set_subtitle(&skin.path.to_string_lossy());

        let load_btn = gtk::Button::builder()
            .label("Load Skin")
            .css_classes(["suggested-action"])
            .valign(gtk::Align::Center)
            .build();

        let unload_btn = gtk::Button::builder()
            .label("Unload Skin")
            .css_classes(["destructive-action"])
            .valign(gtk::Align::Center)
            .build();

        let reload_btn = gtk::Button::builder()
            .label("Reload")
            .valign(gtk::Align::Center)
            .build();

        let ipc = self.ipc.clone();
        let path_clone = skin.path.clone();
        let refresh_helper = self.clone_refresh_helper();
        load_btn.connect_clicked(move |_| {
            let _ = ipc.load_skin(&path_clone);
            refresh_helper();
        });

        let ipc = self.ipc.clone();
        let skin_path_for_unload = skin.path.to_string_lossy().to_string();
        let refresh_helper = self.clone_refresh_helper();
        unload_btn.connect_clicked(move |_| {
            let _ = ipc.unload_skin(&skin_path_for_unload);
            refresh_helper();
        });

        let ipc = self.ipc.clone();
        let skin_path_for_reload = skin.path.to_string_lossy().to_string();
        let refresh_helper = self.clone_refresh_helper();
        reload_btn.connect_clicked(move |_| {
            let _ = ipc.refresh_skin(&skin_path_for_reload);
            refresh_helper();
        });

        let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        btn_box.append(&load_btn);
        btn_box.append(&unload_btn);
        btn_box.append(&reload_btn);
        action_row.add_suffix(&btn_box);
        banner_group.add(&action_row);
        content_box.append(&banner_group);

        // Window & Layout Preferences
        let layout_group = adw::PreferencesGroup::new();
        layout_group.set_title("Window and Layout Configuration");

        if let Some(config) = &skin.config {
            let rainmeter_sec = config.raw_sections.get("rainmeter");
            let mut cur_x = rainmeter_sec
                .and_then(|s| s.get("windowx").or_else(|| s.get("skinx")))
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0);
            let mut cur_y = rainmeter_sec
                .and_then(|s| s.get("windowy").or_else(|| s.get("skiny")))
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0);

            let skin_path_str = skin.path.to_string_lossy().to_string();
            if let Ok(active_skins) = self.ipc.list_skins() {
                for act in active_skins {
                    if act.path == skin_path_str
                        || act.id.eq_ignore_ascii_case(&skin.name)
                        || skin_path_str.contains(&act.id)
                    {
                        cur_x = act.bounds.x as f64;
                        cur_y = act.bounds.y as f64;
                        break;
                    }
                }
            }

            let spin_x = adw::SpinRow::new(Some(&gtk::Adjustment::new(cur_x, -10000.0, 20000.0, 1.0, 100.0, 0.0)), 1.0, 0);
            spin_x.set_title("Window X (Horizontal Offset)");

            let spin_y = adw::SpinRow::new(Some(&gtk::Adjustment::new(cur_y, -10000.0, 20000.0, 1.0, 100.0, 0.0)), 1.0, 0);
            spin_y.set_title("Window Y (Vertical Offset)");

            // Guard prevents the programmatic initial set from triggering IPC calls
            let init_guard = std::rc::Rc::new(std::cell::Cell::new(true));

            let path_for_x = skin.path.clone();
            let ipc_x = self.ipc.clone();
            let skin_id_x = skin_path_str.clone();
            let spin_y_clone = spin_y.clone();
            let guard_x = init_guard.clone();
            spin_x.connect_value_notify(move |row| {
                if guard_x.get() { return; }
                let x = row.value() as i32;
                let y = spin_y_clone.value() as i32;
                let _ = write_key_value_to_file(&path_for_x, "Rainmeter", "WindowX", &x.to_string());
                let _ = ipc_x.set_position(&skin_id_x, x, y);
            });

            let path_for_y = skin.path.clone();
            let ipc_y = self.ipc.clone();
            let skin_id_y = skin_path_str.clone();
            let spin_x_clone = spin_x.clone();
            let guard_y = init_guard.clone();
            spin_y.connect_value_notify(move |row| {
                if guard_y.get() { return; }
                let y = row.value() as i32;
                let x = spin_x_clone.value() as i32;
                let _ = write_key_value_to_file(&path_for_y, "Rainmeter", "WindowY", &y.to_string());
                let _ = ipc_y.set_position(&skin_id_y, x, y);
            });

            // Lift the initialization guard after widgets are fully set up
            init_guard.set(false);

            layout_group.add(&spin_x);
            layout_group.add(&spin_y);

            let opacity_row = adw::ActionRow::new();
            opacity_row.set_title("Window Opacity");
            opacity_row.set_subtitle("Adjust transparency of the desktop widget (10% - 100%)");

            let opacity_scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.1, 1.0, 0.05);
            opacity_scale.set_value(1.0);
            opacity_scale.set_size_request(200, -1);
            opacity_scale.set_draw_value(true);

            let ipc_op = self.ipc.clone();
            let skin_id_op = skin_path_str;
            opacity_scale.connect_value_changed(move |sc| {
                let val = sc.value();
                let _ = ipc_op.set_opacity(&skin_id_op, val);
            });

            opacity_row.add_suffix(&opacity_scale);
            layout_group.add(&opacity_row);
        }
        content_box.append(&layout_group);

        // Variables Editor
        if let Some(config) = &skin.config {
            let vars_group = adw::PreferencesGroup::new();
            vars_group.set_title("Live Variables and Customization");
            vars_group.set_description(Some("Adjust variables in real-time. Changes are pushed live to the desktop skin."));

            for (var_name, var_val) in config.variables.iter() {
                let lower_var = var_name.to_ascii_lowercase();

                // Check if variable is numeric or scale
                if lower_var == "scale" || (var_val.parse::<f64>().is_ok() && lower_var.contains("scale")) {
                    let num_val = var_val.parse::<f64>().unwrap_or(1.0);
                    let scale_row = adw::ActionRow::new();
                    scale_row.set_title(var_name);
                    scale_row.set_subtitle(&format!("Current: {}", var_val));

                    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.5, 3.0, 0.05);
                    scale.set_value(num_val);
                    scale.set_size_request(200, -1);
                    scale.set_draw_value(true);

                    let ipc = self.ipc.clone();
                    let skin_id = skin.name.clone();
                    let v_name = var_name.to_string();
                    let path_for_scale = skin.path.clone();

                    scale.connect_value_changed(move |s| {
                        let new_val = format!("{:.2}", s.value());
                        let _ = ipc.set_variable(&skin_id, &v_name, &new_val);
                        let _ = write_key_value_to_file(&path_for_scale, "Variables", &v_name, &new_val);
                    });

                    scale_row.add_suffix(&scale);
                    vars_group.add(&scale_row);
                } else {
                    let entry_row = adw::EntryRow::new();
                    entry_row.set_title(var_name);
                    entry_row.set_text(var_val);

                    let ipc = self.ipc.clone();
                    let skin_id = skin.name.clone();
                    let v_name = var_name.to_string();
                    let path_for_entry = skin.path.clone();

                    entry_row.connect_changed(move |row| {
                        let new_val = row.text().to_string();
                        let _ = ipc.set_variable(&skin_id, &v_name, &new_val);
                        let _ = write_key_value_to_file(&path_for_entry, "Variables", &v_name, &new_val);
                    });

                    vars_group.add(&entry_row);
                }
            }
            content_box.append(&vars_group);
        }

        scroll.set_child(Some(&content_box));
        self.detail_box.append(&scroll);
    }
}

struct StudioWindowWeak {
    window: adw::ApplicationWindow,
    ipc: Rc<IpcClient>,
    active_list: gtk::ListBox,
    library_box: gtk::Box,
    detail_box: gtk::Box,
    status_label: gtk::Label,
    daemon_banner: adw::Banner,
    search_entry: gtk::SearchEntry,
    search_query: Rc<RefCell<String>>,
    skins: Rc<RefCell<BTreeMap<String, Vec<DiscoveredSkin>>>>,
    selected_skin: Rc<RefCell<Option<DiscoveredSkin>>>,
}

impl StudioWindowWeak {
    fn upgrade(&self) -> Option<StudioWindow> {
        Some(StudioWindow {
            window: self.window.clone(),
            ipc: self.ipc.clone(),
            active_list: self.active_list.clone(),
            library_box: self.library_box.clone(),
            detail_box: self.detail_box.clone(),
            status_label: self.status_label.clone(),
            daemon_banner: self.daemon_banner.clone(),
            search_entry: self.search_entry.clone(),
            search_query: self.search_query.clone(),
            skins: self.skins.clone(),
            selected_skin: self.selected_skin.clone(),
        })
    }
}

