// SPDX-License-Identifier: Apache-2.0
//! Native Windows host. All HWNDs and GDI resources stay on the UI thread.
//! The table is owner-data: notifications read only the bounded retained sample.
#![allow(non_snake_case)]
use crate::{
    display_text,
    supervisor::{worker_path, Adapter, Task},
    Document, THEMES,
};
use std::{
    cell::{Cell, UnsafeCell},
    ffi::c_void,
    mem::{size_of, zeroed},
    path::PathBuf,
    ptr,
};
type H = isize;
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
struct Msg {
    hwnd: H,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    pt: Point,
    private: u32,
}
#[repr(C)]
struct WndClass {
    size: u32,
    style: u32,
    proc: unsafe extern "system" fn(H, u32, usize, isize) -> isize,
    extra: i32,
    window_extra: i32,
    instance: H,
    icon: H,
    cursor: H,
    background: H,
    menu: *const u16,
    class: *const u16,
    small_icon: H,
}
#[repr(C)]
struct Create {
    params: *mut c_void,
    instance: H,
    menu: H,
    parent: H,
    cy: i32,
    cx: i32,
    y: i32,
    x: i32,
    style: i32,
    name: *const u16,
    class: *const u16,
    ex: u32,
}
#[repr(C)]
struct CommonControls {
    size: u32,
    classes: u32,
}
#[repr(C)]
struct NmHdr {
    hwnd: H,
    id: usize,
    code: u32,
}
#[repr(C)]
struct LvItem {
    mask: u32,
    item: i32,
    sub: i32,
    state: u32,
    state_mask: u32,
    text: *mut u16,
    text_max: i32,
    image: i32,
    param: isize,
    indent: i32,
    group: i32,
    columns: u32,
    col_ptr: *mut u32,
    col_fmt: *mut i32,
    group_index: i32,
}
#[repr(C)]
struct LvInfo {
    hdr: NmHdr,
    item: LvItem,
}
#[repr(C)]
struct LvColumn {
    mask: u32,
    fmt: i32,
    width: i32,
    text: *mut u16,
    text_max: i32,
    sub: i32,
    image: i32,
    order: i32,
    min: i32,
    default_width: i32,
    ideal: i32,
}
#[repr(C)]
struct OpenFile {
    size: u32,
    owner: H,
    instance: H,
    filter: *const u16,
    custom: *mut u16,
    custom_max: u32,
    filter_index: u32,
    file: *mut u16,
    file_max: u32,
    file_title: *mut u16,
    file_title_max: u32,
    initial: *const u16,
    title: *const u16,
    flags: u32,
    offset: u16,
    extension: u16,
    default_ext: *const u16,
    data: isize,
    hook: usize,
    template: *const u16,
    reserved: *mut c_void,
    reserved_dw: u32,
    flags_ex: u32,
}
#[repr(C)]
struct MinMax {
    reserved: Point,
    max_size: Point,
    max_position: Point,
    min_track: Point,
    max_track: Point,
}
#[repr(C)]
struct HighContrast {
    size: u32,
    flags: u32,
    scheme: *mut u16,
}
#[link(name = "user32")]
extern "system" {
    fn RegisterClassExW(c: *const WndClass) -> u16;
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: H,
        menu: H,
        instance: H,
        param: *mut c_void,
    ) -> H;
    fn DefWindowProcW(h: H, m: u32, w: usize, l: isize) -> isize;
    fn DestroyWindow(h: H) -> i32;
    fn IsWindow(h: H) -> i32;
    fn ShowWindow(h: H, n: i32) -> i32;
    fn GetMessageW(m: *mut Msg, h: H, min: u32, max: u32) -> i32;
    fn TranslateMessage(m: *const Msg) -> i32;
    fn DispatchMessageW(m: *const Msg) -> isize;
    fn IsDialogMessageW(h: H, m: *mut Msg) -> i32;
    fn PostQuitMessage(n: i32);
    fn SendMessageW(h: H, m: u32, w: usize, l: isize) -> isize;
    fn SetWindowLongPtrW(h: H, index: i32, value: isize) -> isize;
    fn GetWindowLongPtrW(h: H, index: i32) -> isize;
    fn SetWindowTextW(h: H, s: *const u16) -> i32;
    fn GetClientRect(h: H, r: *mut Rect) -> i32;
    fn MoveWindow(h: H, x: i32, y: i32, w: i32, height: i32, repaint: i32) -> i32;
    fn InvalidateRect(h: H, r: *const Rect, erase: i32) -> i32;
    fn RedrawWindow(h: H, r: *const Rect, region: H, flags: u32) -> i32;
    fn SetTimer(h: H, id: usize, ms: u32, proc: usize) -> usize;
    fn KillTimer(h: H, id: usize) -> i32;
    fn LoadCursorW(instance: H, name: *const u16) -> H;
    fn GetKeyState(key: i32) -> i16;
    fn SetFocus(h: H) -> H;
    fn EnableWindow(h: H, enable: i32) -> i32;
    fn MessageBoxW(h: H, text: *const u16, title: *const u16, flags: u32) -> i32;
    fn FillRect(dc: H, rect: *const Rect, brush: H) -> i32;
    fn GetSysColor(index: i32) -> u32;
    fn SystemParametersInfoW(action: u32, param: u32, value: *mut c_void, flags: u32) -> i32;
    fn SetProcessDpiAwarenessContext(context: H) -> i32;
    fn GetDpiForWindow(h: H) -> u32;
    fn SetWindowPos(h: H, after: H, x: i32, y: i32, w: i32, height: i32, flags: u32) -> i32;
    fn OpenClipboard(h: H) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn SetClipboardData(format: u32, data: H) -> H;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(s: *const u16) -> H;
    fn GlobalAlloc(flags: u32, bytes: usize) -> H;
    fn GlobalLock(h: H) -> *mut c_void;
    fn GlobalUnlock(h: H) -> i32;
    fn GlobalFree(h: H) -> H;
}
#[link(name = "gdi32")]
extern "system" {
    fn CreateSolidBrush(color: u32) -> H;
    fn DeleteObject(h: H) -> i32;
    fn CreateFontW(
        height: i32,
        width: i32,
        esc: i32,
        orient: i32,
        weight: i32,
        italic: u32,
        under: u32,
        strike: u32,
        charset: u32,
        out: u32,
        clip: u32,
        quality: u32,
        pitch: u32,
        face: *const u16,
    ) -> H;
    fn SetTextColor(dc: H, color: u32) -> u32;
    fn SetBkColor(dc: H, color: u32) -> u32;
    fn AddFontResourceExW(path: *const u16, flags: u32, reserved: *mut c_void) -> i32;
    fn RemoveFontResourceExW(path: *const u16, flags: u32, reserved: *mut c_void) -> i32;
}
#[link(name = "comctl32")]
extern "system" {
    fn InitCommonControlsEx(c: *const CommonControls) -> i32;
}
#[link(name = "comdlg32")]
extern "system" {
    fn GetOpenFileNameW(o: *mut OpenFile) -> i32;
    fn CommDlgExtendedError() -> u32;
}
#[link(name = "shell32")]
extern "system" {
    fn DragAcceptFiles(h: H, accept: i32);
    fn DragQueryFileW(drop: H, index: u32, file: *mut u16, max: u32) -> u32;
    fn DragFinish(drop: H);
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn color(rgb: u32) -> u32 {
    ((rgb & 255) << 16) | (rgb & 0xff00) | ((rgb >> 16) & 255)
}
unsafe fn settext(h: H, s: &str) {
    SetWindowTextW(h, wide(s).as_ptr());
}
#[derive(Clone, Copy, Default)]
struct Paint {
    details: H,
    title: H,
    status: H,
    bg: u32,
    panel: u32,
    fg: u32,
    accent: u32,
    info: u32,
    brush: H,
    panel_brush: H,
    trim: u32,
    dpi: i32,
}
struct Host {
    busy: Cell<bool>,
    paint: Cell<Paint>,
    ui: UnsafeCell<Ui>,
}
struct Ui {
    paint: *const Cell<Paint>,
    dirty: bool,
    adapter: Adapter,
    doc: Document,
    path: Option<PathBuf>,
    task: Option<(u64, Task)>,
    theme: usize,
    dpi: i32,
    window: H,
    open: H,
    reload: H,
    cancel: H,
    combo: H,
    about: H,
    copy: H,
    title: H,
    status: H,
    table: H,
    details: H,
    font: H,
    mono: H,
    heading: H,
    brush: H,
    panel_brush: H,
    bg: u32,
    panel: u32,
    fg: u32,
    accent: u32,
    info: u32,
    font_files: Vec<Vec<u16>>,
    columns: usize,
}
impl Ui {
    fn new(adapter: Adapter, theme: usize) -> Self {
        Self{paint:ptr::null(),dirty:true,adapter,doc:Document{status:"Open or drop one local file. Read-only; no COM registration needed. First slice: 64 MiB source limit.".into(),..Document::default()},path:None,task:None,theme,dpi:96,window:0,open:0,reload:0,cancel:0,combo:0,about:0,copy:0,title:0,status:0,table:0,details:0,font:0,mono:0,heading:0,brush:0,panel_brush:0,bg:0,panel:0,fg:0,accent:0,info:0,font_files:Vec::new(),columns:0}
    }
    unsafe fn child(&self, class: &str, label: &str, style: u32, id: isize) -> H {
        CreateWindowExW(
            0,
            wide(class).as_ptr(),
            wide(label).as_ptr(),
            0x50000000 | style,
            0,
            0,
            0,
            0,
            self.window,
            id,
            GetModuleHandleW(ptr::null()),
            ptr::null_mut(),
        )
    }
    unsafe fn create(&mut self) {
        self.open = self.child("BUTTON", "&Open file…", 0x10000, 101);
        self.reload = self.child("BUTTON", "&Reload", 0x10000, 102);
        self.cancel = self.child("BUTTON", "&Cancel", 0x10000, 103);
        self.combo = self.child("COMBOBOX", "Theme", 0x10000 | 0x200000 | 3, 104);
        self.about = self.child("BUTTON", "&About", 0x10000, 105);
        self.copy = self.child("BUTTON", "Copy shown &row", 0x10000, 106);
        self.title = self.child("STATIC", self.adapter.title, 0, 107);
        self.status = self.child("STATIC", &self.doc.status, 0, 108);
        self.table = self.child(
            "SysListView32",
            "Sample table",
            0x10000 | 0x100000 | 0x200000 | 1 | 0x1000 | 8,
            109,
        );
        SendMessageW(self.table, 0x1036, 0, 0x20 | 0x10000); // full row selection and double buffer
        self.details=self.child("EDIT","Open a file to view coverage and structured metadata.\r\n\r\nSix base designs; all data stays local.\r\n\r\nCtrl+O: Open   F5: Reload   Esc: Cancel\r\nCtrl+C: Copy shown row (table focus)\r\nTab: move between controls",0x10000|0x200000|0x100000|4|0x800|0x40,110);
        for t in THEMES {
            SendMessageW(self.combo, 0x143, 0, wide(t.name).as_ptr() as isize);
        }
        SendMessageW(self.combo, 0x14e, self.theme, 0);
        DragAcceptFiles(self.window, 1);
        SetTimer(self.window, 1, 80, 0);
        self.apply_theme();
        self.layout();
        SetFocus(self.open);
    }
    unsafe fn layout(&self) {
        let mut r = Rect::default();
        GetClientRect(self.window, &mut r);
        let q = |n: i32| n * self.dpi / 96;
        let w = r.right;
        let h = r.bottom;
        for (control, x, width) in [
            (self.open, 16, 115),
            (self.reload, 139, 85),
            (self.cancel, 232, 85),
            (self.combo, 337, 145),
            (self.copy, 500, 145),
            (self.about, 663, 80),
        ] {
            MoveWindow(
                control,
                q(x),
                q(16),
                q(width),
                q(if control == self.combo { 230 } else { 32 }),
                1,
            );
        }
        MoveWindow(self.title, q(18), q(66), w - q(36), q(42), 1);
        MoveWindow(self.status, q(18), h - q(66), w - q(36), q(56), 1);
        let side = q(310);
        let tablew = (w - side - q(52)).max(q(200));
        MoveWindow(
            self.table,
            q(16),
            q(119),
            tablew,
            (h - q(200)).max(q(50)),
            1,
        );
        MoveWindow(
            self.details,
            q(32) + tablew,
            q(119),
            side,
            (h - q(200)).max(q(50)),
            1,
        );
    }
    unsafe fn apply_theme(&mut self) {
        let t = &THEMES[self.theme];
        let mut hc = HighContrast {
            size: size_of::<HighContrast>() as u32,
            flags: 0,
            scheme: ptr::null_mut(),
        };
        SystemParametersInfoW(0x42, hc.size, (&mut hc as *mut HighContrast).cast(), 0);
        let forced = hc.flags & 1 != 0;
        self.bg = if forced {
            GetSysColor(5)
        } else {
            color(t.background)
        };
        self.panel = if forced {
            GetSysColor(5)
        } else {
            color(t.panel)
        };
        self.fg = if forced {
            GetSysColor(8)
        } else {
            color(t.text)
        };
        if self.brush != 0 {
            DeleteObject(self.brush);
        }
        if self.panel_brush != 0 {
            DeleteObject(self.panel_brush);
        }
        self.accent = if forced { self.fg } else { color(t.accent) };
        self.info = if forced { self.fg } else { color(t.info) };
        self.brush = CreateSolidBrush(self.bg);
        self.panel_brush = CreateSolidBrush(self.panel);
        let mut hc_trim = HighContrast {
            size: size_of::<HighContrast>() as u32,
            flags: 0,
            scheme: ptr::null_mut(),
        };
        SystemParametersInfoW(
            0x42,
            hc_trim.size,
            (&mut hc_trim as *mut HighContrast).cast(),
            0,
        );
        (*self.paint).set(Paint {
            details: self.details,
            title: self.title,
            status: self.status,
            bg: self.bg,
            panel: self.panel,
            fg: self.fg,
            accent: self.accent,
            info: self.info,
            brush: self.brush,
            panel_brush: self.panel_brush,
            trim: if hc_trim.flags & 1 != 0 {
                self.fg
            } else if self.theme == 5 {
                color(0xe08d3c)
            } else {
                self.accent
            },
            dpi: self.dpi,
        });
        self.dirty = true;
        // Private process-local fonts. Missing font files use installed system fallbacks.
        if self.font_files.is_empty() {
            if let Ok(exe) = std::env::current_exe() {
                if let Some(dir) = exe.parent() {
                    for name in ["Cinzel.ttf", "Outfit.ttf", "JetBrainsMono.ttf"] {
                        let p = dir.join("fonts").join(name);
                        if p.is_file() {
                            use std::os::windows::ffi::OsStrExt;
                            let w: Vec<u16> = p.as_os_str().encode_wide().chain(Some(0)).collect();
                            if AddFontResourceExW(w.as_ptr(), 0x10, ptr::null_mut()) != 0 {
                                self.font_files.push(w);
                            }
                        }
                    }
                }
            }
        }
        let branded = self.theme >= 3 && !self.font_files.is_empty() && !forced;
        let ui = if branded { "Outfit" } else { "Segoe UI" };
        let data = if branded {
            "JetBrains Mono"
        } else if self.theme == 2 {
            "MS Gothic"
        } else {
            "Consolas"
        };
        let make = |size: i32, face: &str, weight: i32| {
            CreateFontW(
                -size * self.dpi / 96,
                0,
                0,
                0,
                weight,
                0,
                0,
                0,
                1,
                0,
                0,
                5,
                0,
                wide(face).as_ptr(),
            )
        };
        let old = [self.font, self.mono, self.heading];
        self.font = make(17, ui, 400);
        self.mono = make(16, data, 400);
        self.heading = make(25, if branded { "Cinzel" } else { ui }, 600);
        for h in [
            self.open,
            self.reload,
            self.cancel,
            self.combo,
            self.copy,
            self.about,
            self.status,
        ] {
            SendMessageW(h, 0x30, self.font as usize, 1);
        }
        SendMessageW(self.table, 0x30, self.mono as usize, 1);
        SendMessageW(self.details, 0x30, self.mono as usize, 1);
        SendMessageW(self.title, 0x30, self.heading as usize, 1);
        for f in old {
            if f != 0 {
                DeleteObject(f);
            }
        }
        SendMessageW(self.table, 0x1001, 0, self.panel as isize);
        SendMessageW(self.table, 0x1024, 0, self.fg as isize);
        SendMessageW(self.table, 0x1026, 0, self.panel as isize);
        InvalidateRect(self.window, ptr::null(), 1);
        InvalidateRect(self.details, ptr::null(), 1);
    }
    unsafe fn open_dialog(&mut self) {
        let mut buf = vec![0u16; 32768];
        let filter = wide(if self.adapter.csv_spool {
            "CSV / text files\0*.csv;*.tsv;*.txt\0All files\0*.*\0"
        } else {
            "Parquet files\0*.parquet;*.pq\0All files\0*.*\0"
        });
        let mut o: OpenFile = zeroed();
        o.size = size_of::<OpenFile>() as u32;
        o.owner = self.window;
        o.filter = filter.as_ptr();
        o.file = buf.as_mut_ptr();
        o.file_max = buf.len() as u32;
        o.flags = 0x1000 | 0x800 | 0x80000 | 8;
        if GetOpenFileNameW(&mut o) != 0 {
            use std::os::windows::ffi::OsStringExt;
            let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            self.open_path(std::ffi::OsString::from_wide(&buf[..n]).into());
        } else {
            let e = CommDlgExtendedError();
            if e != 0 {
                self.doc.status = format!("Open dialog failed ({e:#x}); try dropping a file.");
                self.refresh_status();
            }
        }
    }
    unsafe fn open_path(&mut self, path: PathBuf) {
        self.task.take();
        if self.path.as_ref() != Some(&path) {
            self.doc.table = None;
            self.populate();
            settext(self.details, "Opening new document…");
        }
        let generation = self.doc.begin();
        self.path = Some(path.clone());
        settext(
            self.title,
            &format!(
                "{} — {}",
                self.adapter.title,
                display_text(&path.file_name().unwrap_or_default().to_string_lossy(), 160)
            ),
        );
        match worker_path(self.adapter) {
            Ok(worker) => self.task = Some((generation, Task::start(self.adapter, path, worker))),
            Err(e) => {
                self.doc.finish(generation, Err(e));
            }
        }
        self.refresh_status();
    }
    unsafe fn refresh_status(&mut self) {
        self.dirty = true;
        settext(self.status, &self.doc.status);
        EnableWindow(self.cancel, self.task.is_some() as i32);
        EnableWindow(self.copy, self.doc.table.is_some() as i32);
    }
    unsafe fn populate(&mut self) {
        SendMessageW(self.table, 0x102f, 0, 0);
        for _ in 0..self.columns {
            SendMessageW(self.table, 0x101c, 0, 0);
        }
        self.columns = 0;
        if let Some(t) = &self.doc.table {
            for (i, name) in t.columns.iter().enumerate() {
                let mut text = wide(name);
                let mut c: LvColumn = zeroed();
                c.mask = 1 | 2 | 4;
                c.width = if i == 0 { 130 } else { 180 } * self.dpi / 96;
                c.text = text.as_mut_ptr();
                SendMessageW(self.table, 0x1061, i, (&mut c as *mut LvColumn) as isize);
                self.columns += 1;
            }
            SendMessageW(self.table, 0x102f, t.rows.len(), 0);
            settext(self.details, &t.details.replace('\n', "\r\n"));
        }
        InvalidateRect(self.table, ptr::null(), 1);
    }
    unsafe fn poll(&mut self) {
        let result = self
            .task
            .as_mut()
            .and_then(|(g, t)| t.poll().map(|r| (*g, r)));
        if let Some((g, r)) = result {
            self.task.take();
            let success = r.is_ok();
            if self.doc.finish(g, r) && success {
                self.populate();
            }
            self.refresh_status();
        }
    }
    unsafe fn selected(&self) -> Option<usize> {
        let i = SendMessageW(self.table, 0x100c, usize::MAX, 2);
        if i >= 0 {
            Some(i as usize)
        } else {
            None
        }
    }
    unsafe fn inspect_row(&self) {
        if let Some((t, i)) = self.doc.table.as_ref().zip(self.selected()) {
            if let Some(row) = t.rows.get(i) {
                let body = t
                    .columns
                    .iter()
                    .zip(row)
                    .enumerate()
                    .map(|(c, (name, value))| {
                        format!("Column {} · {}\r\n{}\r\n", c + 1, name, value)
                    })
                    .collect::<Vec<_>>()
                    .join("\r\n");
                settext(self.details,&format!("Selected sample row {}\r\nShown values only; … means truncated.\r\n\r\n{}\r\nCoverage / metadata:\r\n{}",i+1,body,t.details.replace('\n',"\r\n")));
            }
        }
    }
    unsafe fn copy_row(&mut self) {
        let Some((t, i)) = self.doc.table.as_ref().zip(self.selected()) else {
            self.doc.status = "Select a sample row first.".into();
            self.refresh_status();
            return;
        };
        let Some(row) = t.rows.get(i) else {
            return;
        };
        let text = wide(&row.join("\t"));
        if OpenClipboard(self.window) == 0 {
            return;
        }
        let h = GlobalAlloc(0x42, text.len() * 2);
        if h != 0 {
            let p = GlobalLock(h);
            if !p.is_null() {
                ptr::copy_nonoverlapping(text.as_ptr(), p.cast::<u16>(), text.len());
                GlobalUnlock(h);
                if EmptyClipboard() != 0 && SetClipboardData(13, h) != 0 {
                    self.doc.status =
                        "Copied shown row only; truncated displays remain truncated.".into();
                } else {
                    GlobalFree(h);
                }
            } else {
                GlobalFree(h);
            }
        }
        CloseClipboard();
        self.refresh_status();
    }
    unsafe fn command(&mut self, id: u16, notification: u16) {
        match id {
            101 => self.open_dialog(),
            102 => {
                if let Some(p) = self.path.clone() {
                    self.open_path(p);
                }
            }
            103 => {
                self.task.take();
                self.doc.begin();
                self.doc.status = "Cancelled. Previous preview, if any, remains visible.".into();
                self.refresh_status();
            }
            104 if notification == 1 => {
                let n = SendMessageW(self.combo, 0x147, 0, 0);
                if n >= 0 && (n as usize) < THEMES.len() {
                    self.theme = n as usize;
                    self.apply_theme();
                }
            }
            105 => {
                MessageBoxW(self.window,wide("Made by Maria\nFree and open source (Apache-2.0).\n\nRead-only, local, worker-backed sample inspector.\nSix base designs; no COM registration required.\nFirst desktop slice: 64 MiB input, 200 rows, 64 shown columns.\nNo edits, full scan or arbitrary paging in this slice.\nPrivate OFL fonts: Cinzel, Outfit, JetBrains Mono when supplied.\nNative accessibility acceptance is pending.").as_ptr(),wide("About standalone viewer").as_ptr(),0);
            }
            106 => self.copy_row(),
            _ => {}
        }
    }
}
unsafe extern "system" fn wndproc(h: H, m: u32, w: usize, l: isize) -> isize {
    if m == 0x81 {
        let c = &*(l as *const Create);
        SetWindowLongPtrW(h, -21, c.params as isize);
        return 1;
    }
    if m == 2 {
        KillTimer(h, 1);
        PostQuitMessage(0);
        return 0;
    }
    let p = GetWindowLongPtrW(h, -21) as *mut Host;
    if p.is_null() {
        return DefWindowProcW(h, m, w, l);
    }
    let host = &*p;
    let paint = host.paint.get();
    if paint.brush != 0 && (m == 0x138 || m == 0x133) {
        SetTextColor(
            w as H,
            if l == paint.title {
                paint.accent
            } else if l == paint.status {
                paint.info
            } else {
                paint.fg
            },
        );
        SetBkColor(
            w as H,
            if l == paint.details {
                paint.panel
            } else {
                paint.bg
            },
        );
        return if l == paint.details {
            paint.panel_brush
        } else {
            paint.brush
        };
    }
    if paint.brush != 0 && m == 0x14 {
        let mut r = Rect::default();
        GetClientRect(h, &mut r);
        FillRect(w as H, &r, paint.brush);
        let trim = CreateSolidBrush(paint.trim);
        FillRect(
            w as H,
            &Rect {
                left: 16 * paint.dpi / 96,
                top: 107 * paint.dpi / 96,
                right: r.right - 16 * paint.dpi / 96,
                bottom: 109 * paint.dpi / 96,
            },
            trim,
        );
        DeleteObject(trim);
        return 1;
    }
    // Synchronous control notifications can re-enter during SendMessage/layout.
    // Never create overlapping mutable references to Ui across that boundary.
    if host.busy.replace(true) {
        return DefWindowProcW(h, m, w, l);
    }
    let s = &mut *host.ui.get();
    let result = match m {
        1 => {
            s.window = h;
            s.dpi = GetDpiForWindow(h).max(96) as i32;
            s.create();
            0
        }
        0x24 => {
            let mm = &mut *(l as *mut MinMax);
            mm.min_track = Point {
                x: 850 * s.dpi / 96,
                y: 440 * s.dpi / 96,
            };
            0
        }
        5 => {
            s.layout();
            0
        }
        0x111 => {
            s.command((w & 0xffff) as u16, (w >> 16) as u16);
            0
        }
        0x113 => {
            s.poll();
            0
        }
        0x112 if w & 0xfff0 == 0xf060 => {
            DestroyWindow(h);
            0
        }
        0x10 => {
            DestroyWindow(h);
            0
        }
        2 => {
            s.task.take();
            KillTimer(h, 1);
            PostQuitMessage(0);
            0
        }
        0x233 => {
            let count = DragQueryFileW(w as H, u32::MAX, ptr::null_mut(), 0);
            if count == 1 {
                let len = DragQueryFileW(w as H, 0, ptr::null_mut(), 0);
                let mut buf = vec![0u16; len as usize + 1];
                DragQueryFileW(w as H, 0, buf.as_mut_ptr(), len + 1);
                use std::os::windows::ffi::OsStringExt;
                let path = std::ffi::OsString::from_wide(&buf[..len as usize]);
                DragFinish(w as H);
                s.open_path(path.into());
            } else {
                DragFinish(w as H);
                s.doc.status =
                    "Drop one file per window. Launch another viewer window for another file."
                        .into();
                s.refresh_status();
            }
            0
        }
        0x4e => {
            let nm = &*(l as *const NmHdr);
            if nm.hwnd == s.table && nm.code == (-177i32) as u32 {
                let info = &mut *(l as *mut LvInfo);
                if info.item.mask & 1 != 0 {
                    if let Some(value) = s
                        .doc
                        .table
                        .as_ref()
                        .and_then(|t| t.rows.get(info.item.item as usize))
                        .and_then(|r| r.get(info.item.sub as usize))
                    {
                        let text = wide(value);
                        let max = (info.item.text_max.max(0) as usize).saturating_sub(1);
                        let n = max.min(text.len() - 1);
                        if !info.item.text.is_null() && info.item.text_max > 0 {
                            ptr::copy_nonoverlapping(text.as_ptr(), info.item.text, n);
                            *info.item.text.add(n) = 0;
                        }
                    }
                }
                0
            } else if nm.hwnd == s.table && nm.code == (-101i32) as u32 {
                s.inspect_row();
                0
            } else {
                DefWindowProcW(h, m, w, l)
            }
        }
        0x2e0 => {
            s.dpi = (w & 0xffff) as i32;
            let r = &*(l as *const Rect);
            SetWindowPos(
                h,
                0,
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                0x14,
            );
            s.apply_theme();
            s.layout();
            0
        }
        0x1a | 0x31a => {
            s.apply_theme();
            0
        }
        _ => DefWindowProcW(h, m, w, l),
    };
    let dirty = s.dirty;
    s.dirty = false;
    host.busy.set(false);
    if dirty && IsWindow(h) != 0 {
        RedrawWindow(h, ptr::null(), 0, 0x185);
    }
    result
}
pub fn error(message: &str) {
    unsafe {
        MessageBoxW(
            0,
            wide(message).as_ptr(),
            wide("Standalone viewer").as_ptr(),
            0x10,
        );
    }
}
pub fn run(adapter: Adapter, args: Vec<std::ffi::OsString>) -> Result<(), String> {
    unsafe {
        let mut initial = None;
        let mut theme = 0;
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            if arg == "--theme" {
                let id = args.next().ok_or("--theme requires an ID")?;
                theme = THEMES
                    .iter()
                    .position(|t| t.id == id.to_string_lossy())
                    .ok_or("Unknown theme; use paper, ink, pc98, doloris, sumimi or marigold")?;
            } else if initial.is_none() {
                initial = Some(PathBuf::from(arg));
            } else {
                return Err("Open one file per window".into());
            }
        }
        SetProcessDpiAwarenessContext(-4);
        InitCommonControlsEx(&CommonControls {
            size: size_of::<CommonControls>() as u32,
            classes: 1,
        });
        let instance = GetModuleHandleW(ptr::null());
        let class = wide(&format!(
            "MariaStandalone{}",
            adapter.title.replace(' ', "")
        ));
        let c = WndClass {
            size: size_of::<WndClass>() as u32,
            style: 3,
            proc: wndproc,
            extra: 0,
            window_extra: 0,
            instance,
            icon: 0,
            cursor: LoadCursorW(0, 32512usize as *const u16),
            background: 0,
            menu: ptr::null(),
            class: class.as_ptr(),
            small_icon: 0,
        };
        if RegisterClassExW(&c) == 0 {
            return Err("Cannot register standalone window class".into());
        }
        let state = Box::into_raw(Box::new(Host {
            busy: Cell::new(false),
            paint: Cell::new(Paint::default()),
            ui: UnsafeCell::new(Ui::new(adapter, theme)),
        }));
        (*(*state).ui.get()).paint = &(*state).paint;
        let h = CreateWindowExW(
            0x10000,
            class.as_ptr(),
            wide(adapter.title).as_ptr(),
            0xcf0000 | 0x10000000,
            0,
            0,
            1200,
            760,
            0,
            0,
            instance,
            state.cast(),
        );
        if h == 0 {
            drop(Box::from_raw(state));
            return Err("Cannot create standalone window".into());
        }
        if let Some(path) = initial {
            (*state).busy.set(true);
            (*(*state).ui.get()).open_path(path);
            (*state).busy.set(false);
            RedrawWindow(h, ptr::null(), 0, 0x185);
        }
        ShowWindow(h, 5);
        let mut msg: Msg = zeroed();
        loop {
            let result = GetMessageW(&mut msg, 0, 0, 0);
            if result <= 0 {
                break;
            }
            let ctrl = GetKeyState(0x11) < 0;
            if msg.message == 0x100 {
                let command = if ctrl && msg.wparam == b'O' as usize {
                    Some(101)
                } else if msg.wparam == 0x74 {
                    Some(102)
                } else if msg.wparam == 0x1b {
                    Some(103)
                } else if ctrl
                    && msg.wparam == b'C' as usize
                    && msg.hwnd == (*(*state).ui.get()).table
                {
                    Some(106)
                } else {
                    None
                };
                if let Some(id) = command {
                    (*state).busy.set(true);
                    (*(*state).ui.get()).command(id, 0);
                    (*state).busy.set(false);
                    RedrawWindow(h, ptr::null(), 0, 0x185);
                    continue;
                }
            }
            if IsDialogMessageW(h, &mut msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        let host = Box::from_raw(state);
        let s = &*host.ui.get();
        for f in [s.font, s.mono, s.heading, s.brush, s.panel_brush] {
            if f != 0 {
                DeleteObject(f);
            }
        }
        for path in &s.font_files {
            RemoveFontResourceExW(path.as_ptr(), 0x10, ptr::null_mut());
        }
        Ok(())
    }
}
