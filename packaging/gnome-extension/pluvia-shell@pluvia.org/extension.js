import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import GLib from 'gi://GLib';

const PLUVIA_WM_CLASS = 'pluvia-widget';
const PLUVIA_APP_ID = 'org.pluvia.widget';

export default class PluviaShellExtension extends Extension {
    enable() {
        this._displaySignals = [];
        this._windowSignals = new Map();
        this._restackTimeoutId = null;

        // Hook into 'window-created' signal on global.display
        const windowCreatedId = global.display.connect('window-created', (_display, window) => {
            this._onWindowCreated(window);
        });
        this._displaySignals.push({ emitter: global.display, id: windowCreatedId });

        // Hook into 'restacked' signal so normal window focus or restacking never places
        // pluvia-widget surfaces above active application windows
        const restackedId = global.display.connect('restacked', () => {
            this._scheduleRestack();
        });
        this._displaySignals.push({ emitter: global.display, id: restackedId });

        // Scan windows already open and mapped upon extension activation
        this._scanExistingWindows();
    }

    disable() {
        // Disconnect display-level signals
        if (this._displaySignals) {
            for (const { emitter, id } of this._displaySignals) {
                if (emitter && id) {
                    emitter.disconnect(id);
                }
            }
            this._displaySignals = [];
        }

        // Disconnect per-window tracking signals
        if (this._windowSignals) {
            for (const [window, ids] of this._windowSignals.entries()) {
                if (window) {
                    for (const id of ids) {
                        try {
                            window.disconnect(id);
                        } catch (_e) {
                            // Window might already be destroyed
                        }
                    }
                }
            }
            this._windowSignals.clear();
        }

        // Cancel pending restack timers
        if (this._restackTimeoutId) {
            GLib.source_remove(this._restackTimeoutId);
            this._restackTimeoutId = null;
        }
    }

    _isPluviaWidget(metaWindow) {
        if (!metaWindow) return false;

        const wmClass = (metaWindow.get_wm_class?.() || '').toLowerCase();
        const wmInstance = (metaWindow.get_wm_class_instance?.() || '').toLowerCase();
        const appId = (metaWindow.get_gtk_application_id?.() || '').toLowerCase();
        const title = (metaWindow.get_title?.() || '').toLowerCase();

        return (
            wmClass === PLUVIA_WM_CLASS ||
            wmInstance === PLUVIA_WM_CLASS ||
            appId === PLUVIA_APP_ID ||
            wmClass === 'pluvia' ||
            wmInstance === 'pluvia' ||
            title.startsWith('pluvia')
        );
    }

    _onWindowCreated(metaWindow) {
        if (!metaWindow) return;

        if (this._isPluviaWidget(metaWindow)) {
            this._enforceWidgetStacking(metaWindow);
        }

        const windowSignals = [];

        // Watch for delayed wm-class or property assignment
        const wmClassNotifyId = metaWindow.connect('notify::wm-class', (win) => {
            if (this._isPluviaWidget(win)) {
                this._enforceWidgetStacking(win);
            }
        });
        windowSignals.push(wmClassNotifyId);

        // Disconnect signals when the window is unmanaged
        const unmanagedId = metaWindow.connect('unmanaged', (win) => {
            this._untrackWindow(win);
        });
        windowSignals.push(unmanagedId);

        this._windowSignals.set(metaWindow, windowSignals);
    }

    _untrackWindow(metaWindow) {
        if (this._windowSignals && this._windowSignals.has(metaWindow)) {
            const ids = this._windowSignals.get(metaWindow);
            for (const id of ids) {
                try {
                    metaWindow.disconnect(id);
                } catch (_e) {
                    // Window might already be destroyed
                }
            }
            this._windowSignals.delete(metaWindow);
        }
    }

    _scanExistingWindows() {
        if (typeof global.get_window_actors !== 'function') return;

        const actors = global.get_window_actors();
        for (const actor of actors) {
            const metaWindow = actor.meta_window ?? actor.get_meta_window?.();
            if (metaWindow && this._isPluviaWidget(metaWindow)) {
                this._enforceWidgetStacking(metaWindow);
            }
        }
    }

    _enforceWidgetStacking(metaWindow) {
        if (!metaWindow) return;

        const actor = metaWindow.get_compositor_private?.() ?? metaWindow.get_actor?.();
        if (!actor) return;

        // Tag the actor as a background widget
        actor._isPluviaBackgroundWidget = true;

        const parent = actor.get_parent?.();
        if (parent && typeof parent.set_child_below_sibling === 'function') {
            // Null sibling places the actor at the absolute bottom of the parent container
            parent.set_child_below_sibling(actor, null);
        }
    }

    _scheduleRestack() {
        if (this._restackTimeoutId) {
            return;
        }

        this._restackTimeoutId = GLib.idle_add(GLib.PRIORITY_DEFAULT_IDLE, () => {
            this._restackTimeoutId = null;
            this._lowerAllWidgets();
            return GLib.SOURCE_REMOVE;
        });
    }

    _lowerAllWidgets() {
        if (typeof global.get_window_actors !== 'function') return;

        const actors = global.get_window_actors();
        for (const actor of actors) {
            const metaWindow = actor.meta_window ?? actor.get_meta_window?.();
            if (metaWindow && this._isPluviaWidget(metaWindow)) {
                const parent = actor.get_parent?.();
                if (parent && typeof parent.set_child_below_sibling === 'function') {
                    parent.set_child_below_sibling(actor, null);
                }
            }
        }
    }
}
