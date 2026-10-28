// SPDX-License-Identifier: Apache-2.0

//! Windows Explorer preview handler (x64, per-user COM registration).
//!
//! This module is compiled only on Windows. The platform-independent preview
//! pipeline and registration plan are tested on Linux; the checklist in
//! TESTING.md lists what must be exercised on real Windows (stream-only input,
//! apartment behavior, blocked files, bitness, registration conflicts).
//!
//! The handler is deliberately read-only: it consumes the supplied `IStream`,
//! displays text, and never writes the source or opens links.

#![allow(non_snake_case, non_upper_case_globals)]

use std::ffi::c_void;
use std::io::{self, Read};
use std::path::PathBuf;
use std::ptr;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Once, OnceLock};
use std::thread;
use std::time::Duration;

use csvview_core::text::render_text;
use csvview_core::{ScanReport, ScanRequest, Settings};

use crate::registration::{
    register_plan, ExpectedValue, RegistryValue, CLSID_CSV_PREVIEW, PREVIEW_HANDLERS_KEY,
};
use crate::{PreviewConfig, PreviewSession, SessionPhase, WorkerBackend};

type HRESULT = i32;
type ULONG = u32;
type HWND = *mut c_void;
type HINSTANCE = *mut c_void;
type HMODULE = *mut c_void;
type HDC = *mut c_void;
type HFONT = *mut c_void;
type HBRUSH = *mut c_void;
type HKEY = *mut c_void;
type LRESULT = isize;
type WPARAM = usize;
type LPARAM = isize;
type UINT = u32;
type DWORD = u32;
type BOOL = i32;
type LPCWSTR = *const u16;
type LPWSTR = *mut u16;
type LONG = i32;

const S_OK: HRESULT = 0;
const S_FALSE: HRESULT = 1;
const E_NOINTERFACE: HRESULT = 0x8000_4002u32 as i32;
const E_POINTER: HRESULT = 0x8000_4003u32 as i32;
const E_UNEXPECTED: HRESULT = 0x8000_FFFFu32 as i32;
const E_FAIL: HRESULT = 0x8000_4005u32 as i32;
const CLASS_E_NOAGGREGATION: HRESULT = 0x8004_0110u32 as i32;
const CLASS_E_CLASSNOTAVAILABLE: HRESULT = 0x8004_0111u32 as i32;
const STG_E_ACCESSDENIED: HRESULT = 0x8003_0005u32 as i32;
const ERROR_ALREADY_EXISTS: HRESULT = 0x8007_00B7u32 as i32;

const STGM_WRITE_BITS: u32 = 0x0000_0003;
const STATFLAG_NONAME: u32 = 0x0000_0001;
const WM_PAINT: UINT = 0x000F;
const WM_ERASEBKGND: UINT = 0x0014;
const WM_KEYDOWN: UINT = 0x0100;
const WM_DESTROY: UINT = 0x0002;
const WM_APP_DONE: UINT = 0x8001;
const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_CLIPCHILDREN: u32 = 0x0200_0000;
const SW_SHOW: i32 = 5;
const GWLP_USERDATA: i32 = -21;
const DT_LEFT: UINT = 0x0000;
const DT_TOP: UINT = 0x0000;
const DT_NOPREFIX: UINT = 0x0800;
const DT_EXPANDTABS: UINT = 0x0040;
const DT_WORDBREAK: UINT = 0x0010;
const TRANSPARENT: i32 = 1;
const DEFAULT_CHARSET: u32 = 1;
const OUT_DEFAULT_PRECIS: u32 = 0;
const CLIP_DEFAULT_PRECIS: u32 = 0;
const CLEARTYPE_QUALITY: u32 = 5;
const FIXED_PITCH: u32 = 1;
const FF_MODERN: u32 = 48;
const FW_NORMAL: i32 = 400;
const ERROR_CLASS_ALREADY_EXISTS: u32 = 1410;
const HKEY_CURRENT_USER: HKEY = 0x8000_0001usize as HKEY;
const KEY_READ: u32 = 0x0002_0019;
const KEY_WRITE: u32 = 0x0002_0006;
const REG_SZ: u32 = 1;
const REG_OPTION_NON_VOLATILE: u32 = 0;
const DLL_PROCESS_ATTACH: DWORD = 1;

static LIVE_COM_OBJECTS: AtomicU32 = AtomicU32::new(0);
static SERVER_LOCKS: AtomicU32 = AtomicU32::new(0);
static DLL_INSTANCE: AtomicUsize = AtomicUsize::new(0);

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GUID {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

const fn guid(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> GUID {
    GUID {
        data1,
        data2,
        data3,
        data4,
    }
}

const IID_IUNKNOWN: GUID = guid(0x0000_0000, 0, 0, [0xC0, 0, 0, 0, 0, 0, 0, 0x46]);
const IID_ICLASSFACTORY: GUID = guid(0x0000_0001, 0, 0, [0xC0, 0, 0, 0, 0, 0, 0, 0x46]);
const IID_IINITIALIZEWITHSTREAM: GUID = guid(
    0xB824_B49D,
    0x22AC,
    0x4161,
    [0xAC, 0x8A, 0x99, 0x16, 0xE8, 0xFA, 0x3F, 0x7F],
);
const IID_IPREVIEWHANDLER: GUID = guid(
    0x8895_B1C6,
    0xB41F,
    0x4C1C,
    [0xA5, 0x62, 0x0D, 0x56, 0x42, 0x50, 0x83, 0x6F],
);

#[repr(C)]
#[derive(Clone, Copy)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct POINT {
    x: i32,
    y: i32,
}

#[repr(C)]
struct MSG {
    hwnd: HWND,
    message: UINT,
    wParam: WPARAM,
    lParam: LPARAM,
    time: DWORD,
    pt: POINT,
}

#[repr(C)]
struct PAINTSTRUCT {
    hdc: HDC,
    fErase: BOOL,
    rcPaint: RECT,
    fRestore: BOOL,
    fIncUpdate: BOOL,
    rgbReserved: [u8; 32],
}

#[repr(C)]
struct WNDCLASSW {
    style: UINT,
    lpfnWndProc: Option<unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT>,
    cbClsExtra: i32,
    cbWndExtra: i32,
    hInstance: HINSTANCE,
    hIcon: *mut c_void,
    hCursor: *mut c_void,
    hbrBackground: HBRUSH,
    lpszMenuName: LPCWSTR,
    lpszClassName: LPCWSTR,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FILETIME {
    dwLowDateTime: DWORD,
    dwHighDateTime: DWORD,
}

#[repr(C)]
struct STATSTG {
    pwcsName: LPWSTR,
    type_: DWORD,
    cbSize: u64,
    mtime: FILETIME,
    ctime: FILETIME,
    atime: FILETIME,
    grfMode: DWORD,
    grfLocksSupported: DWORD,
    clsid: GUID,
    grfStateBits: DWORD,
    reserved: DWORD,
}

#[repr(C)]
struct IStreamVtbl {
    query_interface: *const c_void,
    add_ref: *const c_void,
    release: *const c_void,
    read: unsafe extern "system" fn(*mut c_void, *mut c_void, u32, *mut u32) -> HRESULT,
    write: *const c_void,
    seek: *const c_void,
    set_size: *const c_void,
    copy_to: *const c_void,
    commit: *const c_void,
    revert: *const c_void,
    lock_region: *const c_void,
    unlock_region: *const c_void,
    stat: unsafe extern "system" fn(*mut c_void, *mut STATSTG, u32) -> HRESULT,
    clone: *const c_void,
}

#[repr(C)]
struct IPreviewHandlerVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> ULONG,
    release: unsafe extern "system" fn(*mut c_void) -> ULONG,
    set_window: unsafe extern "system" fn(*mut c_void, HWND) -> HRESULT,
    set_rect: unsafe extern "system" fn(*mut c_void, *const RECT) -> HRESULT,
    do_preview: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    unload: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    set_focus: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    query_focus: unsafe extern "system" fn(*mut c_void, *mut HWND) -> HRESULT,
    translate_accelerator: unsafe extern "system" fn(*mut c_void, *mut MSG) -> HRESULT,
}

#[repr(C)]
struct IInitializeWithStreamVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> ULONG,
    release: unsafe extern "system" fn(*mut c_void) -> ULONG,
    initialize: unsafe extern "system" fn(*mut c_void, *mut c_void, u32) -> HRESULT,
}

#[repr(C)]
struct IClassFactoryVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> ULONG,
    release: unsafe extern "system" fn(*mut c_void) -> ULONG,
    create_instance: unsafe extern "system" fn(
        *mut c_void,
        *mut c_void,
        *const GUID,
        *mut *mut c_void,
    ) -> HRESULT,
    lock_server: unsafe extern "system" fn(*mut c_void, BOOL) -> HRESULT,
}

#[repr(C)]
struct InitInterface {
    vtbl: *const IInitializeWithStreamVtbl,
    owner: *mut PreviewObject,
}

#[repr(C)]
struct PreviewObject {
    vtbl: *const IPreviewHandlerVtbl,
    refcount: AtomicU32,
    stream: *mut c_void,
    parent: HWND,
    rect: RECT,
    window: HWND,
    font: HFONT,
    brush: HBRUSH,
    session: Option<Arc<PreviewSession>>,
    text: Vec<u16>,
    init: InitInterface,
}

static PREVIEW_VTBL: IPreviewHandlerVtbl = IPreviewHandlerVtbl {
    query_interface: preview_query_interface,
    add_ref: preview_add_ref,
    release: preview_release,
    set_window: preview_set_window,
    set_rect: preview_set_rect,
    do_preview: preview_do_preview,
    unload: preview_unload,
    set_focus: preview_set_focus,
    query_focus: preview_query_focus,
    translate_accelerator: preview_translate_accelerator,
};

static INIT_VTBL: IInitializeWithStreamVtbl = IInitializeWithStreamVtbl {
    query_interface: init_query_interface,
    add_ref: init_add_ref,
    release: init_release,
    initialize: init_initialize,
};

#[repr(C)]
struct ClassFactory {
    vtbl: *const IClassFactoryVtbl,
    refcount: AtomicU32,
}

static FACTORY_VTBL: IClassFactoryVtbl = IClassFactoryVtbl {
    query_interface: factory_query_interface,
    add_ref: factory_add_ref,
    release: factory_release,
    create_instance: factory_create_instance,
    lock_server: factory_lock_server,
};

unsafe fn object_from_this(this: *mut c_void) -> *mut PreviewObject {
    this as *mut PreviewObject
}

unsafe fn object_add_ref(object: *mut PreviewObject) -> ULONG {
    (*object).refcount.fetch_add(1, Ordering::SeqCst) + 1
}

unsafe fn object_release(object: *mut PreviewObject) -> ULONG {
    let previous = (*object).refcount.fetch_sub(1, Ordering::SeqCst);
    if previous == 1 {
        LIVE_COM_OBJECTS.fetch_sub(1, Ordering::SeqCst);
        drop(Box::from_raw(object));
        0
    } else {
        previous - 1
    }
}

unsafe fn query_preview_interface(
    object: *mut PreviewObject,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    *ppv = ptr::null_mut();
    let iid = *riid;
    if iid == IID_IUNKNOWN || iid == IID_IPREVIEWHANDLER {
        object_add_ref(object);
        *ppv = object as *mut c_void;
        S_OK
    } else if iid == IID_IINITIALIZEWITHSTREAM {
        object_add_ref(object);
        *ppv = &mut (*object).init as *mut InitInterface as *mut c_void;
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn preview_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    query_preview_interface(object_from_this(this), riid, ppv)
}

unsafe extern "system" fn preview_add_ref(this: *mut c_void) -> ULONG {
    object_add_ref(object_from_this(this))
}

unsafe extern "system" fn preview_release(this: *mut c_void) -> ULONG {
    object_release(object_from_this(this))
}

unsafe extern "system" fn init_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    let interface = &*(this as *const InitInterface);
    query_preview_interface(interface.owner, riid, ppv)
}

unsafe extern "system" fn init_add_ref(this: *mut c_void) -> ULONG {
    let interface = &*(this as *const InitInterface);
    object_add_ref(interface.owner)
}

unsafe extern "system" fn init_release(this: *mut c_void) -> ULONG {
    let interface = &*(this as *const InitInterface);
    object_release(interface.owner)
}

unsafe fn stream_add_ref(stream: *mut c_void) {
    if stream.is_null() {
        return;
    }
    let vtbl = *(stream as *mut *const IStreamVtbl);
    let add_ref: unsafe extern "system" fn(*mut c_void) -> ULONG =
        std::mem::transmute((*vtbl).add_ref);
    add_ref(stream);
}

unsafe fn stream_release(stream: *mut c_void) {
    if stream.is_null() {
        return;
    }
    let vtbl = *(stream as *mut *const IStreamVtbl);
    let release: unsafe extern "system" fn(*mut c_void) -> ULONG =
        std::mem::transmute((*vtbl).release);
    release(stream);
}

/// Reads an Explorer-supplied `IStream` on the bounded preview worker. The
/// host must keep the stream valid for the lifetime of the handler; COM
/// apartment/isolation behavior is on the real-Windows test checklist.
struct StreamReader {
    stream: *mut c_void,
}

impl StreamReader {
    unsafe fn new(stream: *mut c_void) -> Self {
        stream_add_ref(stream);
        Self { stream }
    }
}

impl Read for StreamReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let mut read = 0u32;
        let vtbl = unsafe { *(self.stream as *mut *const IStreamVtbl) };
        let result = unsafe {
            ((*vtbl).read)(
                self.stream,
                buffer.as_mut_ptr() as *mut c_void,
                buffer.len().min(u32::MAX as usize) as u32,
                &mut read,
            )
        };
        if result < 0 {
            Err(io::Error::other(format!(
                "IStream::Read failed: 0x{result:08X}"
            )))
        } else {
            Ok(read as usize)
        }
    }
}

// The stream is consumed by one bounded spool thread. See TESTING.md for the
// required apartment and stream-lifetime checks.
unsafe impl Send for StreamReader {}

impl Drop for StreamReader {
    fn drop(&mut self) {
        unsafe { stream_release(self.stream) };
    }
}

unsafe fn stream_size_hint(stream: *mut c_void) -> Option<u64> {
    if stream.is_null() {
        return None;
    }
    let vtbl = *(stream as *mut *const IStreamVtbl);
    let mut stat: STATSTG = std::mem::zeroed();
    let result = ((*vtbl).stat)(stream, &mut stat, STATFLAG_NONAME);
    if result < 0 {
        None
    } else {
        Some(stat.cbSize)
    }
}

unsafe extern "system" fn init_initialize(
    this: *mut c_void,
    stream: *mut c_void,
    mode: u32,
) -> HRESULT {
    if stream.is_null() {
        return E_POINTER;
    }
    if mode & STGM_WRITE_BITS != 0 {
        return STG_E_ACCESSDENIED;
    }
    let interface = &*(this as *const InitInterface);
    let object = &mut *interface.owner;
    if !object.stream.is_null() {
        stream_release(object.stream);
    }
    stream_add_ref(stream);
    object.stream = stream;
    S_OK
}

unsafe extern "system" fn preview_set_window(this: *mut c_void, parent: HWND) -> HRESULT {
    let object = &mut *object_from_this(this);
    object.parent = parent;
    if !object.window.is_null() && !parent.is_null() {
        SetParent(object.window, parent);
    }
    S_OK
}

unsafe extern "system" fn preview_set_rect(this: *mut c_void, rect: *const RECT) -> HRESULT {
    if rect.is_null() {
        return E_POINTER;
    }
    let object = &mut *object_from_this(this);
    object.rect = *rect;
    if !object.window.is_null() {
        let width = (object.rect.right - object.rect.left).max(1);
        let height = (object.rect.bottom - object.rect.top).max(1);
        MoveWindow(
            object.window,
            object.rect.left,
            object.rect.top,
            width,
            height,
            1,
        );
    }
    S_OK
}

unsafe extern "system" fn preview_set_focus(this: *mut c_void) -> HRESULT {
    let object = &*object_from_this(this);
    if !object.window.is_null() {
        SetFocus(object.window);
    }
    S_OK
}

unsafe extern "system" fn preview_query_focus(_this: *mut c_void, focus: *mut HWND) -> HRESULT {
    if focus.is_null() {
        return E_POINTER;
    }
    *focus = GetFocus();
    S_OK
}

unsafe extern "system" fn preview_translate_accelerator(
    _this: *mut c_void,
    _message: *mut MSG,
) -> HRESULT {
    S_FALSE
}

unsafe extern "system" fn preview_unload(this: *mut c_void) -> HRESULT {
    let object = &mut *object_from_this(this);
    object.cleanup();
    S_OK
}

impl PreviewObject {
    unsafe fn cleanup(&mut self) {
        if let Some(session) = self.session.take() {
            session.cancel();
        }
        if !self.window.is_null() {
            DestroyWindow(self.window);
            self.window = ptr::null_mut();
        }
        if !self.stream.is_null() {
            stream_release(self.stream);
            self.stream = ptr::null_mut();
        }
        if !self.font.is_null() {
            DeleteObject(self.font);
            self.font = ptr::null_mut();
        }
        if !self.brush.is_null() {
            DeleteObject(self.brush);
            self.brush = ptr::null_mut();
        }
        self.text.clear();
    }
}

impl Drop for PreviewObject {
    fn drop(&mut self) {
        unsafe { self.cleanup() };
    }
}

unsafe fn ensure_window_class() {
    static REGISTER: Once = Once::new();
    REGISTER.call_once(|| {
        let class_name = CLASS_NAME.get_or_init(|| wide("CsvViewerPreview"));
        let window_class = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(preview_window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: GetModuleHandleW(ptr::null()),
            hIcon: ptr::null_mut(),
            hCursor: ptr::null_mut(),
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        let atom = RegisterClassW(&window_class);
        if atom == 0 {
            let error = GetLastError();
            if error != ERROR_CLASS_ALREADY_EXISTS {
                // Painting still falls back to an empty window; do not panic in
                // the Explorer host.
                eprintln!("csv-preview: RegisterClassW failed: {error}");
            }
        }
    });
}

unsafe extern "system" fn preview_do_preview(this: *mut c_void) -> HRESULT {
    let object = &mut *object_from_this(this);
    if object.stream.is_null() || object.parent.is_null() {
        return E_UNEXPECTED;
    }
    if let Some(session) = object.session.take() {
        session.cancel();
    }
    if !object.window.is_null() {
        DestroyWindow(object.window);
        object.window = ptr::null_mut();
    }

    let mut request = ScanRequest::preview();
    let (settings, _) = Settings::load(None);
    settings.apply_to(&mut request);
    let reader = StreamReader::new(object.stream);
    let length_hint = stream_size_hint(object.stream);
    let session =
        match PreviewSession::start(reader, length_hint, &request, PreviewConfig::default()) {
            Ok(session) => Arc::new(session),
            Err(error) => {
                object.set_error_text(&format!("CSV preview could not start: {error}"));
                return E_FAIL;
            }
        };
    object.session = Some(session);
    object.refresh_text();

    ensure_window_class();
    let width = (object.rect.right - object.rect.left).max(320);
    let height = (object.rect.bottom - object.rect.top).max(200);
    let class_name = wide("CsvViewerPreview");
    let title = wide("CSV viewer and preview");
    let window = CreateWindowExW(
        0,
        class_name.as_ptr(),
        title.as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN,
        object.rect.left,
        object.rect.top,
        width,
        height,
        object.parent,
        ptr::null_mut(),
        GetModuleHandleW(ptr::null()),
        object as *mut PreviewObject as *mut c_void,
    );
    if window.is_null() {
        object.set_error_text("CSV preview could not create its child window.");
        return E_FAIL;
    }
    object.window = window;
    SetWindowLongPtrW(window, GWLP_USERDATA, object as *mut PreviewObject as isize);
    ShowWindow(window, SW_SHOW);
    UpdateWindow(window);
    S_OK
}

impl PreviewObject {
    unsafe fn set_error_text(&mut self, message: &str) {
        self.text = wide_text(&format!(
            "CSV viewer and preview — Made by Maria\n\n{message}\n\nRead-only."
        ));
        if !self.window.is_null() {
            InvalidateRect(self.window, ptr::null(), 1);
        }
    }

    unsafe fn refresh_text(&mut self) {
        let Some(session) = &self.session else {
            return;
        };
        let snapshot = session.snapshot();
        let report: Option<Arc<ScanReport>> = snapshot
            .full_report
            .clone()
            .or_else(|| snapshot.preview.clone());
        let Some(report) = report else {
            return;
        };
        let mut text = render_text(&report, 110, 38);
        match snapshot.phase {
            SessionPhase::Scanning => text.push_str("\n\nFull scan running…"),
            SessionPhase::TimedOut => {
                text.push_str("\n\nFull scan timed out; the partial preview is unchanged.")
            }
            SessionPhase::Failed => text.push_str(&format!(
                "\n\nFull scan failed: {}",
                snapshot.error.as_deref().unwrap_or("unknown error")
            )),
            SessionPhase::Complete => text.push_str(
                "\n\nFull scan complete. Only the explicit scan result is shown as a total.",
            ),
            _ => {
                if !report.complete {
                    text.push_str(
                        "\n\nThis is a bounded preview. Press F for an explicit full scan; it is bounded by worker limits.",
                    );
                }
            }
        }
        self.text = wide_text(&text);
        if !self.window.is_null() {
            InvalidateRect(self.window, ptr::null(), 1);
        }
    }
}

unsafe fn start_full_scan(object: &mut PreviewObject, window: HWND) {
    let Some(session) = object.session.clone() else {
        object.set_error_text("The preview stream is no longer available.");
        return;
    };
    if session.snapshot().phase == SessionPhase::Scanning {
        return;
    }
    let Some(worker) = worker_path() else {
        object.set_error_text(
            "csv-worker.exe was not found next to the preview DLL; full scan is unavailable.",
        );
        return;
    };
    let mut request = ScanRequest::full();
    let (settings, _) = Settings::load(None);
    settings.apply_to(&mut request);
    let timeout = Duration::from_millis(request.limits.deadline_ms.unwrap_or(120_000));
    if let Err(error) =
        session.start_full_scan(Arc::new(WorkerBackend::new(worker)), request, timeout)
    {
        object.set_error_text(&format!("Full scan could not start: {error}"));
        return;
    }
    let waiting = session.clone();
    let window_value = window as usize;
    thread::spawn(move || {
        let _ = waiting.wait_for_scan(timeout + Duration::from_secs(1));
        PostMessageW(window_value as HWND, WM_APP_DONE, 0, 0);
    });
    object.refresh_text();
}

static CLASS_NAME: OnceLock<Vec<u16>> = OnceLock::new();

unsafe extern "system" fn preview_window_proc(
    window: HWND,
    message: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let object = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut PreviewObject;
    match message {
        WM_PAINT => {
            paint_window(window, object);
            0
        }
        WM_ERASEBKGND => 1,
        WM_KEYDOWN if wparam == b'F' as usize || wparam == b'f' as usize => {
            if !object.is_null() {
                start_full_scan(&mut *object, window);
            }
            0
        }
        WM_APP_DONE => {
            if !object.is_null() {
                (*object).refresh_text();
                InvalidateRect(window, ptr::null(), 1);
            }
            0
        }
        WM_DESTROY => 0,
        _ => DefWindowProcW(window, message, wparam, lparam),
    }
}

unsafe fn paint_window(window: HWND, object: *mut PreviewObject) {
    let mut paint: PAINTSTRUCT = std::mem::zeroed();
    let device = BeginPaint(window, &mut paint);
    if device.is_null() {
        return;
    }
    let mut client: RECT = std::mem::zeroed();
    GetClientRect(window, &mut client);
    if !object.is_null() {
        let object = &mut *object;
        if object.font.is_null() {
            object.font = CreateFontW(
                -14,
                0,
                0,
                0,
                FW_NORMAL,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                FIXED_PITCH | FF_MODERN,
                wide("Consolas").as_ptr(),
            );
        }
        if object.brush.is_null() {
            object.brush = CreateSolidBrush(0x00FF_FFFF);
        }
        if !object.brush.is_null() {
            FillRect(device, &client, object.brush);
        }
        if !object.font.is_null() && !object.text.is_empty() {
            SelectObject(device, object.font);
            SetBkMode(device, TRANSPARENT);
            SetTextColor(device, 0x0020_2020);
            let mut rect = client;
            rect.left += 8;
            rect.top += 8;
            rect.right -= 8;
            rect.bottom -= 8;
            DrawTextW(
                device,
                object.text.as_ptr(),
                -1,
                &mut rect,
                DT_LEFT | DT_TOP | DT_NOPREFIX | DT_EXPANDTABS | DT_WORDBREAK,
            );
        }
    }
    EndPaint(window, &paint);
}

#[no_mangle]
pub extern "system" fn DllMain(instance: HMODULE, reason: DWORD, _reserved: *mut c_void) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        DLL_INSTANCE.store(instance as usize, Ordering::SeqCst);
    }
    1
}

unsafe fn this_module() -> HMODULE {
    let stored = DLL_INSTANCE.load(Ordering::SeqCst) as HMODULE;
    if !stored.is_null() {
        stored
    } else {
        GetModuleHandleW(ptr::null())
    }
}

unsafe fn module_path() -> Option<PathBuf> {
    let mut buffer = vec![0u16; 32_768];
    let module = this_module();
    let length = GetModuleFileNameW(module, buffer.as_mut_ptr(), buffer.len() as u32);
    if length == 0 || length as usize >= buffer.len() {
        return None;
    }
    Some(PathBuf::from(String::from_utf16_lossy(
        &buffer[..length as usize],
    )))
}

unsafe fn worker_path() -> Option<PathBuf> {
    module_path()?
        .parent()
        .map(|directory| directory.join("csv-worker.exe"))
        .filter(|path| path.is_file())
}

#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    clsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if clsid.is_null() || riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    *ppv = ptr::null_mut();
    let expected = parse_guid(CLSID_CSV_PREVIEW);
    if expected.map(|value| value != *clsid).unwrap_or(true) {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let factory = Box::into_raw(Box::new(ClassFactory {
        vtbl: &FACTORY_VTBL,
        refcount: AtomicU32::new(1),
    }));
    LIVE_COM_OBJECTS.fetch_add(1, Ordering::SeqCst);
    let result = query_factory(factory, riid, ppv);
    release_factory(factory);
    result
}

#[no_mangle]
pub unsafe extern "system" fn DllCanUnloadNow() -> HRESULT {
    if LIVE_COM_OBJECTS.load(Ordering::SeqCst) == 0 && SERVER_LOCKS.load(Ordering::SeqCst) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}

#[no_mangle]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    unsafe {
        let Some(path) = module_path() else {
            return E_FAIL;
        };
        register_module(&path.to_string_lossy(), false)
    }
}

#[no_mangle]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    unregister_module()
}

unsafe fn query_factory(
    factory: *mut ClassFactory,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    *ppv = ptr::null_mut();
    let iid = *riid;
    if iid == IID_IUNKNOWN || iid == IID_ICLASSFACTORY {
        factory_add_ref(factory as *mut c_void);
        *ppv = factory as *mut c_void;
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn factory_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    query_factory(this as *mut ClassFactory, riid, ppv)
}

unsafe extern "system" fn factory_add_ref(this: *mut c_void) -> ULONG {
    let factory = &*(this as *const ClassFactory);
    factory.refcount.fetch_add(1, Ordering::SeqCst) + 1
}

unsafe fn release_factory(factory: *mut ClassFactory) -> ULONG {
    let previous = (*factory).refcount.fetch_sub(1, Ordering::SeqCst);
    if previous == 1 {
        LIVE_COM_OBJECTS.fetch_sub(1, Ordering::SeqCst);
        drop(Box::from_raw(factory));
        0
    } else {
        previous - 1
    }
}

unsafe extern "system" fn factory_release(this: *mut c_void) -> ULONG {
    release_factory(this as *mut ClassFactory)
}

unsafe extern "system" fn factory_create_instance(
    this: *mut c_void,
    outer: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    let _factory = &*(this as *const ClassFactory);
    if !outer.is_null() {
        return CLASS_E_NOAGGREGATION;
    }
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    *ppv = ptr::null_mut();
    let mut object = Box::new(PreviewObject {
        vtbl: &PREVIEW_VTBL,
        refcount: AtomicU32::new(1),
        stream: ptr::null_mut(),
        parent: ptr::null_mut(),
        rect: RECT {
            left: 0,
            top: 0,
            right: 640,
            bottom: 480,
        },
        window: ptr::null_mut(),
        font: ptr::null_mut(),
        brush: ptr::null_mut(),
        session: None,
        text: Vec::new(),
        init: InitInterface {
            vtbl: &INIT_VTBL,
            owner: ptr::null_mut(),
        },
    });
    object.init.owner = &mut *object as *mut PreviewObject;
    LIVE_COM_OBJECTS.fetch_add(1, Ordering::SeqCst);
    let raw = Box::into_raw(object);
    let result = query_preview_interface(raw, riid, ppv);
    object_release(raw);
    result
}

unsafe extern "system" fn factory_lock_server(_this: *mut c_void, lock: BOOL) -> HRESULT {
    if lock != 0 {
        SERVER_LOCKS.fetch_add(1, Ordering::SeqCst);
    } else {
        SERVER_LOCKS.fetch_sub(1, Ordering::SeqCst);
    }
    S_OK
}

fn parse_guid(text: &str) -> Option<GUID> {
    let text = text.trim().trim_matches(|ch| ch == '{' || ch == '}');
    let mut parts = text.split('-');
    let data1 = u32::from_str_radix(parts.next()?, 16).ok()?;
    let data2 = u16::from_str_radix(parts.next()?, 16).ok()?;
    let data3 = u16::from_str_radix(parts.next()?, 16).ok()?;
    let fourth = parts.next()?;
    let fifth = parts.next()?;
    if parts.next().is_some() || fourth.len() != 4 || fifth.len() != 12 {
        return None;
    }
    let mut data4 = [0u8; 8];
    for (index, byte) in fourth
        .as_bytes()
        .chunks(2)
        .chain(fifth.as_bytes().chunks(2))
        .enumerate()
    {
        if index >= 8 {
            return None;
        }
        data4[index] = u8::from_str_radix(std::str::from_utf8(byte).ok()?, 16).ok()?;
    }
    Some(GUID {
        data1,
        data2,
        data3,
        data4,
    })
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wide_text(value: &str) -> Vec<u16> {
    value
        .replace('\n', "\r\n")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

// ---------------------------------------------------------------------------
// Per-user registration
// ---------------------------------------------------------------------------

fn register_module(module: &str, force: bool) -> HRESULT {
    unsafe {
        let plan = register_plan(module);
        for entry in &plan {
            if entry.is_association() && !force {
                if let Some(existing) = read_default(&entry.key) {
                    if !existing.is_empty() && !existing.eq_ignore_ascii_case(CLSID_CSV_PREVIEW) {
                        eprintln!(
                            "csv-preview: refusing to replace preview handler {existing:?} for {}; \
                             ask the user before changing it",
                            entry.key
                        );
                        return ERROR_ALREADY_EXISTS;
                    }
                }
            }
        }
        for entry in &plan {
            if let Err(error) = write_value(entry) {
                eprintln!("csv-preview: registration failed: {error}");
                return E_FAIL;
            }
        }
        S_OK
    }
}

fn unregister_module() -> HRESULT {
    unsafe {
        let module = module_path()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        for entry in register_plan(&module) {
            if value_matches(&entry) {
                let _ = delete_value(&entry.key, entry.name);
            }
        }
        // Do not delete keys whose creation cannot be attributed to this install.
        S_OK
    }
}

fn registry_path(key: &str) -> String {
    if key == PREVIEW_HANDLERS_KEY {
        key.to_owned()
    } else {
        format!("Software\\Classes\\{key}")
    }
}

unsafe fn read_default(key: &str) -> Option<String> {
    let full = wide(&registry_path(key));
    let mut handle: HKEY = ptr::null_mut();
    if RegOpenKeyExW(HKEY_CURRENT_USER, full.as_ptr(), 0, KEY_READ, &mut handle) != 0 {
        return None;
    }
    let mut value_type = 0u32;
    let mut length = 0u32;
    let status = RegQueryValueExW(
        handle,
        ptr::null(),
        ptr::null_mut(),
        &mut value_type,
        ptr::null_mut(),
        &mut length,
    );
    if status != 0 || length == 0 {
        RegCloseKey(handle);
        return None;
    }
    let mut buffer = vec![0u8; length as usize];
    let status = RegQueryValueExW(
        handle,
        ptr::null(),
        ptr::null_mut(),
        &mut value_type,
        buffer.as_mut_ptr(),
        &mut length,
    );
    RegCloseKey(handle);
    if status != 0 {
        return None;
    }
    let utf16: Vec<u16> = buffer
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect();
    Some(String::from_utf16_lossy(&utf16))
}

unsafe fn write_value(entry: &RegistryValue) -> Result<(), String> {
    let full = registry_path(&entry.key);
    let handle = create_key(&full)?;
    let name = entry.name.map(wide);
    let value = wide(&entry.value);
    let status = RegSetValueExW(
        handle,
        name.as_ref()
            .map(|name| name.as_ptr())
            .unwrap_or(ptr::null()),
        0,
        REG_SZ,
        value.as_ptr() as *const u8,
        (value.len() * 2) as u32,
    );
    RegCloseKey(handle);
    if status == 0 {
        Ok(())
    } else {
        Err(format!("RegSetValueExW failed with {status}"))
    }
}

unsafe fn delete_value(key: &str, name: Option<&'static str>) -> Result<(), String> {
    let full = wide(&registry_path(key));
    let mut handle: HKEY = ptr::null_mut();
    if RegOpenKeyExW(HKEY_CURRENT_USER, full.as_ptr(), 0, KEY_WRITE, &mut handle) != 0 {
        return Ok(());
    }
    let name = name.map(wide);
    let status = RegDeleteValueW(
        handle,
        name.as_ref()
            .map(|name| name.as_ptr())
            .unwrap_or(ptr::null()),
    );
    RegCloseKey(handle);
    if status == 0 || status == 2 {
        Ok(())
    } else {
        Err(format!("RegDeleteValueW failed with {status}"))
    }
}

unsafe fn value_matches(entry: &RegistryValue) -> bool {
    let Some(existing) = read_value(&entry.key, entry.name) else {
        return false;
    };
    match entry.expected {
        ExpectedValue::Clsid => existing.eq_ignore_ascii_case(CLSID_CSV_PREVIEW),
        ExpectedValue::ModulePath => existing.eq_ignore_ascii_case(&entry.value),
        ExpectedValue::Static(value) => existing == value,
    }
}

unsafe fn read_value(key: &str, name: Option<&str>) -> Option<String> {
    let full = wide(&registry_path(key));
    let mut handle: HKEY = ptr::null_mut();
    if RegOpenKeyExW(HKEY_CURRENT_USER, full.as_ptr(), 0, KEY_READ, &mut handle) != 0 {
        return None;
    }
    let name = name.map(wide);
    let mut value_type = 0u32;
    let mut length = 0u32;
    let status = RegQueryValueExW(
        handle,
        name.as_ref()
            .map(|name| name.as_ptr())
            .unwrap_or(ptr::null()),
        ptr::null_mut(),
        &mut value_type,
        ptr::null_mut(),
        &mut length,
    );
    if status != 0 || length == 0 {
        RegCloseKey(handle);
        return None;
    }
    let mut buffer = vec![0u8; length as usize];
    let status = RegQueryValueExW(
        handle,
        name.as_ref()
            .map(|name| name.as_ptr())
            .unwrap_or(ptr::null()),
        ptr::null_mut(),
        &mut value_type,
        buffer.as_mut_ptr(),
        &mut length,
    );
    RegCloseKey(handle);
    if status != 0 {
        return None;
    }
    let utf16: Vec<u16> = buffer
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect();
    Some(String::from_utf16_lossy(&utf16))
}

/// Create a key and any missing parents.
unsafe fn create_key(path: &str) -> Result<HKEY, String> {
    if let Some(separator) = path.rfind('\\') {
        let parent_path = &path[..separator];
        let leaf = &path[separator + 1..];
        let parent = create_key(parent_path)?;
        let leaf = wide(leaf);
        let mut handle: HKEY = ptr::null_mut();
        let mut disposition = 0u32;
        let status = RegCreateKeyExW(
            parent,
            leaf.as_ptr(),
            0,
            ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE | KEY_READ,
            ptr::null_mut(),
            &mut handle,
            &mut disposition,
        );
        RegCloseKey(parent);
        if status == 0 {
            Ok(handle)
        } else {
            Err(format!("RegCreateKeyExW failed with {status}"))
        }
    } else {
        let full = wide(path);
        let mut handle: HKEY = ptr::null_mut();
        let mut disposition = 0u32;
        let status = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            full.as_ptr(),
            0,
            ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE | KEY_READ,
            ptr::null_mut(),
            &mut handle,
            &mut disposition,
        );
        if status == 0 {
            Ok(handle)
        } else {
            Err(format!("RegCreateKeyExW failed with {status}"))
        }
    }
}

// ---------------------------------------------------------------------------
// Win32 imports
// ---------------------------------------------------------------------------

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: LPCWSTR) -> HMODULE;
    fn GetModuleFileNameW(module: HMODULE, filename: LPWSTR, size: DWORD) -> DWORD;
    fn GetLastError() -> DWORD;
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(class: *const WNDCLASSW) -> u16;
    fn CreateWindowExW(
        ex_style: DWORD,
        class_name: LPCWSTR,
        window_name: LPCWSTR,
        style: DWORD,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: HWND,
        menu: *mut c_void,
        instance: HINSTANCE,
        param: *mut c_void,
    ) -> HWND;
    fn DefWindowProcW(window: HWND, message: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT;
    fn DestroyWindow(window: HWND) -> BOOL;
    fn ShowWindow(window: HWND, command: i32) -> BOOL;
    fn UpdateWindow(window: HWND) -> BOOL;
    fn MoveWindow(window: HWND, x: i32, y: i32, width: i32, height: i32, repaint: BOOL) -> BOOL;
    fn SetParent(window: HWND, new_parent: HWND) -> HWND;
    fn SetFocus(window: HWND) -> HWND;
    fn GetFocus() -> HWND;
    fn GetClientRect(window: HWND, rect: *mut RECT) -> BOOL;
    fn InvalidateRect(window: HWND, rect: *const RECT, erase: BOOL) -> BOOL;
    fn BeginPaint(window: HWND, paint: *mut PAINTSTRUCT) -> HDC;
    fn EndPaint(window: HWND, paint: *const PAINTSTRUCT) -> BOOL;
    fn FillRect(device: HDC, rect: *const RECT, brush: HBRUSH) -> i32;
    fn DrawTextW(device: HDC, text: LPCWSTR, length: i32, rect: *mut RECT, format: UINT) -> i32;
    fn PostMessageW(window: HWND, message: UINT, wparam: WPARAM, lparam: LPARAM) -> BOOL;
    fn SetWindowLongPtrW(window: HWND, index: i32, value: isize) -> isize;
    fn GetWindowLongPtrW(window: HWND, index: i32) -> isize;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateFontW(
        height: i32,
        width: i32,
        escapement: i32,
        orientation: i32,
        weight: i32,
        italic: DWORD,
        underline: DWORD,
        strike_out: DWORD,
        charset: DWORD,
        out_precision: DWORD,
        clip_precision: DWORD,
        quality: DWORD,
        pitch_and_family: DWORD,
        face: LPCWSTR,
    ) -> HFONT;
    fn SelectObject(device: HDC, object: *mut c_void) -> *mut c_void;
    fn DeleteObject(object: *mut c_void) -> BOOL;
    fn SetTextColor(device: HDC, color: u32) -> u32;
    fn SetBkMode(device: HDC, mode: i32) -> i32;
    fn CreateSolidBrush(color: u32) -> HBRUSH;
}

#[link(name = "advapi32")]
extern "system" {
    fn RegCreateKeyExW(
        key: HKEY,
        sub_key: LPCWSTR,
        reserved: DWORD,
        class: LPCWSTR,
        options: DWORD,
        desired: DWORD,
        security: *mut c_void,
        result: *mut HKEY,
        disposition: *mut DWORD,
    ) -> LONG;
    fn RegOpenKeyExW(
        key: HKEY,
        sub_key: LPCWSTR,
        options: DWORD,
        desired: DWORD,
        result: *mut HKEY,
    ) -> LONG;
    fn RegSetValueExW(
        key: HKEY,
        value_name: LPCWSTR,
        reserved: DWORD,
        value_type: DWORD,
        data: *const u8,
        data_size: DWORD,
    ) -> LONG;
    fn RegQueryValueExW(
        key: HKEY,
        value_name: LPCWSTR,
        reserved: *mut DWORD,
        value_type: *mut DWORD,
        data: *mut u8,
        data_size: *mut DWORD,
    ) -> LONG;
    fn RegDeleteValueW(key: HKEY, value_name: LPCWSTR) -> LONG;
    fn RegCloseKey(key: HKEY) -> LONG;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_registration_clsid() {
        let guid = parse_guid(CLSID_CSV_PREVIEW).unwrap();
        assert_eq!(guid.data1, 0x8f6a0f5e);
        assert_eq!(guid.data4[7], 0x60);
    }
}
