//! System tray integration.
//!
//! Windows: the app starts in the system tray (the console window is hidden
//! right away) and keeps running there; minimizing the window keeps it out
//! of the taskbar as well. A left click on the tray icon shows the window
//! again, a right click opens a small context menu (Show / Exit). The tray
//! icon is the qFlipper icon embedded into the executable by build.rs.
//! Starting a second copy of the app brings the running instance to the
//! front and exits the new copy.
//!
//! Other platforms: no-op.

#[cfg(target_os = "windows")]
mod win {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    const WM_COMMAND: u32 = 0x0111;
    const WM_LBUTTONUP: u32 = 0x0202;
    const WM_RBUTTONUP: u32 = 0x0205;
    const WM_TRAY_ICON: u32 = 0x8001; // WM_APP + 1

    const NIM_ADD: u32 = 0x00;
    const NIM_DELETE: u32 = 0x01;

    const NIF_MESSAGE: u32 = 0x01;
    const NIF_ICON: u32 = 0x02;
    const NIF_TIP: u32 = 0x04;

    const SW_HIDE: i32 = 0x00;
    const SW_RESTORE: i32 = 0x09;

    const ERROR_ALREADY_EXISTS: u32 = 183;

    /// Title of the console window; doubles as the handle a second instance
    /// uses to find the running one
    const APP_WINDOW_TITLE: &str = "Flipper PC Monitor";

    /// Resource ID of the qFlipper icon embedded into the exe by build.rs
    /// (`winresource::set_icon` always uses ID 1)
    const APP_ICON_ID: u32 = 1;
    /// Fallback icon if the embedded one cannot be loaded
    const IDI_APPLICATION: u32 = 32512;

    const MF_STRING: u32 = 0x00;
    const TPM_RIGHTBUTTON: u32 = 0x02;
    const PM_REMOVE: u32 = 0x01;

    const HWND_MESSAGE: *mut c_void = -3isize as *mut c_void;

    const MENU_ID_SHOW: usize = 0xB100;
    const MENU_ID_EXIT: usize = 0xB101;

    const TRAY_ICON_ID: u32 = 1;
    const TRAY_TIP: &str = "Flipper PC Monitor";
    const POLL_INTERVAL: Duration = Duration::from_millis(100);

    static TRAY_HWND: AtomicUsize = AtomicUsize::new(0);

    /// Handle of the single-instance mutex (kept open for the whole lifetime
    /// of the process so the mutex stays owned)
    static SINGLE_INSTANCE_MUTEX: AtomicUsize = AtomicUsize::new(0);

    type WndProc =
        unsafe extern "system" fn(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> isize;

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct Msg {
        hwnd: *mut c_void,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        pt: Point,
    }

    #[repr(C)]
    struct WndClassExW {
        cb_size: u32,
        style: u32,
        lpfn_wnd_proc: WndProc,
        cb_cls_extra: i32,
        cb_wnd_extra: i32,
        h_instance: *mut c_void,
        h_icon: *mut c_void,
        h_cursor: *mut c_void,
        hbr_background: *mut c_void,
        lpsz_menu_name: *const u16,
        lpsz_class_name: *const u16,
        h_icon_sm: *mut c_void,
    }

    #[repr(C)]
    struct NotifyIconDataW {
        cb_size: u32,
        hwnd: *mut c_void,
        u_id: u32,
        u_flags: u32,
        u_callback_message: u32,
        h_icon: *mut c_void,
        sz_tip: [u16; 128],
        dw_state: u32,
        dw_state_mask: u32,
        sz_info: [u16; 256],
        u_timeout: u32,
        sz_info_title: [u16; 64],
        dw_info_flags: u32,
        guid_item: u128,
        h_balloon_icon: *mut c_void,
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetConsoleWindow() -> *mut c_void;
        fn IsIconic(hwnd: *mut c_void) -> i32;
        fn ShowWindow(hwnd: *mut c_void, n_cmd_show: i32) -> i32;
        fn SetForegroundWindow(hwnd: *mut c_void) -> i32;
        fn SetConsoleTitleW(title: *const u16) -> i32;
        fn FindWindowW(class: *const u16, name: *const u16) -> *mut c_void;
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
        fn RegisterClassExW(class: *const WndClassExW) -> u16;
        fn CreateWindowExW(
            ex_style: u32,
            class: *const u16,
            name: *const u16,
            style: u32,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            parent: *mut c_void,
            menu: *mut c_void,
            instance: *mut c_void,
            param: *mut c_void,
        ) -> *mut c_void;
        fn PeekMessageW(
            msg: *mut Msg,
            hwnd: *mut c_void,
            min_filter: u32,
            max_filter: u32,
            remove: u32,
        ) -> i32;
        fn TranslateMessage(msg: *const Msg) -> i32;
        fn DispatchMessageW(msg: *const Msg) -> isize;
        fn DefWindowProcW(
            hwnd: *mut c_void,
            msg: u32,
            wparam: usize,
            lparam: isize,
        ) -> isize;
        fn CreatePopupMenu() -> *mut c_void;
        fn AppendMenuW(menu: *mut c_void, flags: u32, item_id: usize, text: *const u16) -> i32;
        fn TrackPopupMenu(
            menu: *mut c_void,
            flags: u32,
            x: i32,
            y: i32,
            reserved: u32,
            hwnd: *mut c_void,
        ) -> i32;
        fn DestroyMenu(menu: *mut c_void) -> i32;
        fn GetCursorPos(point: *mut Point) -> i32;
    }

    #[link(name = "shell32")]
    extern "system" {
        fn Shell_NotifyIconW(message: u32, data: *const NotifyIconDataW) -> i32;
        fn LoadIconW(instance: *mut c_void, icon_name: u32) -> *mut c_void;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateMutexW(attrs: *const c_void, initial_owner: i32, name: *const u16) -> *mut c_void;
        fn GetLastError() -> u32;
    }

    /// Starts the tray thread. Must be called once at startup.
    pub(super) fn init() {
        // A second copy exits (after bringing the running instance forward)
        unsafe {
            ensure_single_instance();
        }

        unsafe {
            // Give the console window a known title so a second instance can
            // find it
            SetConsoleTitleW(wide(APP_WINDOW_TITLE).as_ptr());
        }

        // Start in the tray: hide the console window right away, the app
        // is reached from the system tray icon from here on
        unsafe {
            let console = GetConsoleWindow();
            if !console.is_null() {
                ShowWindow(console, SW_HIDE);
            }
        }

        if std::thread::Builder::new()
            .name("tray".to_string())
            .spawn(|| unsafe { run() })
            .is_err()
        {
            eprintln!("[tray] Failed to start tray thread");
        }
    }

    /// Enforces single-instance behavior. If another copy of this app is
    /// already running, restores and foregrounds its window and exits this
    /// process. Returns normally when this is the first instance.
    unsafe fn ensure_single_instance() {
        let name = wide("Local\\FlipperPcMonitor");
        let mutex = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        // Must be read before any other Win32 call
        let error = GetLastError();
        if mutex.is_null() {
            // No mutex available; don't block startup over it
            return;
        }
        SINGLE_INSTANCE_MUTEX.store(mutex as usize, Ordering::Relaxed);
        if error != ERROR_ALREADY_EXISTS {
            return;
        }

        // A second copy: show the running instance, then exit
        let other = FindWindowW(std::ptr::null(), wide(APP_WINDOW_TITLE).as_ptr());
        if !other.is_null() {
            ShowWindow(other, SW_RESTORE);
            SetForegroundWindow(other);
        }
        let console = GetConsoleWindow();
        if !console.is_null() {
            ShowWindow(console, SW_HIDE);
        }
        std::process::exit(0);
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16()
            .chain(std::iter::once(0))
            .collect()
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: *mut c_void,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize {
        match msg {
            WM_TRAY_ICON => match lparam as u32 {
                // Left click: bring the window back (restore, no maximize)
                WM_LBUTTONUP => show_console_window(),
                // Right click: context menu
                WM_RBUTTONUP => show_tray_menu(hwnd),
                _ => {}
            },
            WM_COMMAND => match wparam {
                MENU_ID_SHOW => show_console_window(),
                MENU_ID_EXIT => exit_app(),
                _ => {}
            },
            _ => {}
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    /// Shows the console window and brings it to the front.
    unsafe fn show_console_window() {
        let console = GetConsoleWindow();
        if console.is_null() {
            return;
        }
        ShowWindow(console, SW_RESTORE);
        SetForegroundWindow(console);
    }

    unsafe fn show_tray_menu(hwnd: *mut c_void) {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        let show_label = wide("Show");
        let exit_label = wide("Exit");
        AppendMenuW(menu, MF_STRING, MENU_ID_SHOW, show_label.as_ptr());
        AppendMenuW(menu, MF_STRING, MENU_ID_EXIT, exit_label.as_ptr());

        let mut pos = Point { x: 0, y: 0 };
        GetCursorPos(&mut pos);
        // Required so the menu closes when clicking anywhere outside of it
        SetForegroundWindow(hwnd);
        TrackPopupMenu(menu, TPM_RIGHTBUTTON, pos.x, pos.y, 0, hwnd);
        DestroyMenu(menu);
    }

    unsafe fn exit_app() {
        let hwnd = TRAY_HWND.load(Ordering::Relaxed);
        if hwnd != 0 {
            let mut data: NotifyIconDataW = std::mem::zeroed();
            data.cb_size = std::mem::size_of::<NotifyIconDataW>() as u32;
            data.hwnd = hwnd as *mut c_void;
            data.u_id = TRAY_ICON_ID;
            let _ = Shell_NotifyIconW(NIM_DELETE, &data);
        }
        std::process::exit(0);
    }

    /// Runs the message loop of the hidden tray window: pumps shell messages
    /// and watches the console window for the minimized state.
    unsafe fn run() {
        let class_name = wide("FpcmTrayWindow");
        let class = WndClassExW {
            cb_size: std::mem::size_of::<WndClassExW>() as u32,
            style: 0,
            lpfn_wnd_proc: wnd_proc,
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            h_instance: GetModuleHandleW(std::ptr::null()),
            h_icon: std::ptr::null_mut(),
            h_cursor: std::ptr::null_mut(),
            hbr_background: std::ptr::null_mut(),
            lpsz_menu_name: std::ptr::null(),
            lpsz_class_name: class_name.as_ptr(),
            h_icon_sm: std::ptr::null_mut(),
        };
        if RegisterClassExW(&class) == 0 {
            return;
        }

        // Hidden message-only window used as the tray icon callback target
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            wide("FpcmTray").as_ptr(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null_mut(),
        );
        if hwnd.is_null() {
            return;
        }

        let mut data: NotifyIconDataW = std::mem::zeroed();
        data.cb_size = std::mem::size_of::<NotifyIconDataW>() as u32;
        data.hwnd = hwnd;
        data.u_id = TRAY_ICON_ID;
        data.u_flags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        data.u_callback_message = WM_TRAY_ICON;
        // qFlipper icon embedded into the exe (see build.rs); fall back to
        // the default application icon if it cannot be loaded
        let h_instance = GetModuleHandleW(std::ptr::null());
        let app_icon = LoadIconW(h_instance, APP_ICON_ID);
        data.h_icon = if app_icon.is_null() {
            LoadIconW(h_instance, IDI_APPLICATION)
        } else {
            app_icon
        };
        let tip = wide(TRAY_TIP);
        for (slot, ch) in data.sz_tip.iter_mut().zip(tip.iter()) {
            *slot = *ch;
        }
        if Shell_NotifyIconW(NIM_ADD, &data) == 0 {
            return;
        }
        TRAY_HWND.store(hwnd as usize, Ordering::Relaxed);

        let mut msg: Msg = std::mem::zeroed();
        loop {
            // Minimized console window => hide it from the taskbar,
            // the tray icon takes over from here on
            let console = GetConsoleWindow();
            if !console.is_null() && IsIconic(console) != 0 {
                ShowWindow(console, SW_HIDE);
            }

            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }
}

#[cfg(target_os = "windows")]
pub fn init_tray() {
    win::init();
}

/// System tray support is only available on Windows.
#[cfg(not(target_os = "windows"))]
pub fn init_tray() {
    // no-op
}