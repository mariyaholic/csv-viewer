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

// 07A-13: this FFI layer mirrors the Windows SDK spelling (HWND, HDC,
// HINSTANCE, LRESULT, ...), so clippy::upper_case_acronyms fires on 26
// declarations here (DECISIONS.md 07A-13 ruling: triage into deliberate
// allows, then make clippy strict). Renaming would leave the SDK names the
// surrounding SDK-calling code and TESTING.md text refer to, so the allow
// is whole-file and deliberate; it is not a licence for acronyms elsewhere.
#![allow(clippy::upper_case_acronyms)]
#![allow(non_snake_case, non_upper_case_globals)]

use std::ffi::c_void;
use std::io::{self, Read};
use std::path::PathBuf;
use std::ptr;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::Duration;

use csvview_core::text::render_text;
use csvview_core::{ScanReport, ScanRequest, Settings};

use crate::registration::{
    apply_plan, register_plan, unregister, Registry, RegistryValue, Value, CLSID_CSV_PREVIEW,
    PREVIEW_HANDLERS_KEY,
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
const ERROR_FILE_NOT_FOUND: LONG = 2;
const ERROR_ALREADY_EXISTS: HRESULT = 0x8007_00B7u32 as i32;

const STGM_WRITE_BITS: u32 = 0x0000_0003;
const STATFLAG_NONAME: u32 = 0x0000_0001;
const COINIT_MULTITHREADED: u32 = 0x0;
const WM_SIZE: UINT = 0x0005;
const WM_DPICHANGED: UINT = 0x02E0;
// 07A-10: child panes only receive this; WM_DPICHANGED goes to top-level
// windows and never reaches a WS_CHILD preview pane.
const WM_DPICHANGED_AFTERPARENT: UINT = 0x02E3;
const LOGPIXELSY: i32 = 90;
const WM_PAINT: UINT = 0x000F;
const WM_ERASEBKGND: UINT = 0x0014;
const WM_KEYDOWN: UINT = 0x0100;
// 07A-11: clicking the pane must move keyboard focus into it, or the F
// hotkey can never fire.
const WM_MOUSEACTIVATE: UINT = 0x0021;
const WM_LBUTTONDOWN: UINT = 0x0201;
const MA_ACTIVATE: i32 = 1;
// 07A-11: WM_KEYDOWN's wparam is a virtual-key code. The previous
// comparison used the character literals b'F'/b'f' — and 0x66, the old
// lowercase branch, is VK_NUMPAD6, so Numpad-6 triggered the scan and
// comparison-by-literal never meant anything.
const VK_F: WPARAM = 0x46;
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
const HKEY_CLASSES_ROOT: HKEY = 0x8000_0000usize as HKEY;
const HKEY_CURRENT_USER: HKEY = 0x8000_0001usize as HKEY;
const KEY_READ: u32 = 0x0002_0019;
const KEY_WRITE: u32 = 0x0002_0006;
const REG_SZ: u32 = 1;
const REG_DWORD: u32 = 4;
const REG_OPTION_NON_VOLATILE: u32 = 0;
const DLL_PROCESS_ATTACH: DWORD = 1;

static LIVE_COM_OBJECTS: AtomicU32 = AtomicU32::new(0);
static SERVER_LOCKS: AtomicU32 = AtomicU32::new(0);
static DLL_INSTANCE: AtomicUsize = AtomicUsize::new(0);

#[repr(C)]
struct SIZE {
    cx: i32,
    cy: i32,
}

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
const IID_ISTREAM: GUID = guid(0x0000_000C, 0, 0, [0xC0, 0, 0, 0, 0, 0, 0, 0x46]);
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
const IID_IOBJECTWITHSITE: GUID = guid(
    0xFC48_01A3,
    0x2BA9,
    0x11CF,
    [0xA2, 0x29, 0x00, 0xAA, 0x00, 0x3D, 0x73, 0x52],
);
const IID_IPREVIEWHANDLERFRAME: GUID = guid(
    0xFEC8_7AAF,
    0x35F9,
    0x447A,
    [0xAD, 0x79, 0x43, 0xA1, 0x03, 0xD4, 0x51, 0x4D],
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
#[cfg(test)]
#[allow(clippy::upper_case_acronyms)]
struct WNDCLASSEXW {
    cbSize: UINT,
    style: UINT,
    lpfnWndProc: Option<unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT>,
    cbClsExtra: i32,
    cbWndExtra: i32,
    hInstance: HINSTANCE,
    hIcon: *mut c_void,
    hCursor: *mut c_void,
    hbrBackground: *mut c_void,
    lpszMenuName: LPCWSTR,
    lpszClassName: LPCWSTR,
    hIconSm: *mut c_void,
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

/// The IUnknown core slots; identical at the start of every COM vtable, so
/// generic `QueryInterface`/`Release` dispatch works through any interface.
#[repr(C)]
struct IUnknownVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> ULONG,
    release: unsafe extern "system" fn(*mut c_void) -> ULONG,
}

#[repr(C)]
struct IObjectWithSiteVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> ULONG,
    release: unsafe extern "system" fn(*mut c_void) -> ULONG,
    set_site: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    get_site: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
}

/// The subset of `IPreviewHandlerFrame` the handler calls back into (07A-11).
#[repr(C)]
struct IPreviewHandlerFrameVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> ULONG,
    release: unsafe extern "system" fn(*mut c_void) -> ULONG,
    get_window_context: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    translate_accelerator: unsafe extern "system" fn(*mut c_void, *mut MSG) -> HRESULT,
}

/// `IObjectWithSite` on the preview object itself, mirroring the inventory
/// `InitInterface` pattern (07A-11).
#[repr(C)]
struct SiteInterface {
    vtbl: *const IObjectWithSiteVtbl,
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
    /// `IObjectWithSite` on the object itself (07A-11).
    site: SiteInterface,
    /// The host's `IPreviewHandlerFrame`, queried out of the site in
    /// `SetSite`. An owned reference, released when the object is destroyed.
    frame: *mut c_void,
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

static SITE_VTBL: IObjectWithSiteVtbl = IObjectWithSiteVtbl {
    query_interface: site_query_interface,
    add_ref: site_add_ref,
    release: site_release,
    set_site: site_set_site,
    get_site: site_get_site,
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

// ---------------------------------------------------------------------------
// Panic containment (07A-05)
// ---------------------------------------------------------------------------

// Since Rust 1.81 an unwind out of an `extern "system"` function aborts the
// process — and prevhost.exe may be hosting other preview handlers too.
// Contained here, every Win32 entry point fails the call instead of taking
// the host down with it.

fn contain_hresult(body: impl FnOnce() -> HRESULT) -> HRESULT {
    contain_hresult_or(E_FAIL, body)
}

fn contain_hresult_or(default: HRESULT, body: impl FnOnce() -> HRESULT) -> HRESULT {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)).unwrap_or(default)
}

/// Reference counts fail leak-safe on unwind: a counter stuck high keeps the
/// DLL resident instead of risking a use-after-free.
fn contain_ulong(body: impl FnOnce() -> ULONG) -> ULONG {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)).unwrap_or(1)
}

/// A window proc failure falls back to the default procedure, so the host
/// still processes the message.
fn contain_lresult(
    window: HWND,
    message: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
    body: impl FnOnce() -> LRESULT,
) -> LRESULT {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(body))
        .unwrap_or_else(|_| unsafe { DefWindowProcW(window, message, wparam, lparam) })
}

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
        // Release the host frame we hold before the object's state is dropped.
        let frame = (*object).frame;
        (*object).frame = ptr::null_mut();
        release_interface(frame);
        drop(Box::from_raw(object));
        if LIVE_COM_OBJECTS.load(Ordering::SeqCst) == 0 {
            // The last object is gone, so its windows are destroyed too;
            // the class is ours alone and can be removed.
            unregister_window_class();
        }
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
    } else if iid == IID_IOBJECTWITHSITE {
        object_add_ref(object);
        *ppv = &mut (*object).site as *mut SiteInterface as *mut c_void;
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
    contain_hresult(|| unsafe { query_preview_interface(object_from_this(this), riid, ppv) })
}

unsafe extern "system" fn preview_add_ref(this: *mut c_void) -> ULONG {
    contain_ulong(|| unsafe { object_add_ref(object_from_this(this)) })
}

unsafe extern "system" fn preview_release(this: *mut c_void) -> ULONG {
    contain_ulong(|| unsafe { object_release(object_from_this(this)) })
}

unsafe extern "system" fn init_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    contain_hresult(|| unsafe {
        let interface = &*(this as *const InitInterface);
        query_preview_interface(interface.owner, riid, ppv)
    })
}

unsafe extern "system" fn init_add_ref(this: *mut c_void) -> ULONG {
    contain_ulong(|| unsafe {
        let interface = &*(this as *const InitInterface);
        object_add_ref(interface.owner)
    })
}

unsafe extern "system" fn init_release(this: *mut c_void) -> ULONG {
    contain_ulong(|| unsafe {
        let interface = &*(this as *const InitInterface);
        object_release(interface.owner)
    })
}

/// Call `QueryInterface` through an arbitrary COM interface pointer. The
/// caller owns the returned reference when the call succeeds.
unsafe fn query_interface_ptr(
    interface: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    let table = *(interface as *mut *const IUnknownVtbl);
    ((*table).query_interface)(interface, riid, ppv)
}

/// Call `Release` through an arbitrary COM interface pointer.
unsafe fn release_interface(interface: *mut c_void) {
    if interface.is_null() {
        return;
    }
    let table = *(interface as *mut *const IUnknownVtbl);
    ((*table).release)(interface);
}

unsafe extern "system" fn site_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    contain_hresult(|| unsafe {
        let interface = &*(this as *const SiteInterface);
        query_preview_interface(interface.owner, riid, ppv)
    })
}

unsafe extern "system" fn site_add_ref(this: *mut c_void) -> ULONG {
    contain_ulong(|| unsafe {
        let interface = &*(this as *const SiteInterface);
        object_add_ref(interface.owner)
    })
}

unsafe extern "system" fn site_release(this: *mut c_void) -> ULONG {
    contain_ulong(|| unsafe {
        let interface = &*(this as *const SiteInterface);
        object_release(interface.owner)
    })
}

/// `IObjectWithSite::SetSite` — store the host site and query it for the
/// `IPreviewHandlerFrame` the accelerator forwarder calls back into
/// (07A-11, mirroring parquet-viewer L5).
unsafe extern "system" fn site_set_site(this: *mut c_void, site: *mut c_void) -> HRESULT {
    contain_hresult(|| unsafe {
        let interface = &*(this as *const SiteInterface);
        let object = interface.owner;
        // Replace any previous site's frame reference.
        release_interface((*object).frame);
        (*object).frame = ptr::null_mut();
        if site.is_null() {
            // Explorer passes NULL to tear the site down.
            return S_OK;
        }
        let mut frame: *mut c_void = ptr::null_mut();
        let hr = query_interface_ptr(site, &IID_IPREVIEWHANDLERFRAME, &mut frame);
        if hr == S_OK && !frame.is_null() {
            (*object).frame = frame;
        }
        // Accept the site even when it exposes no frame: the host may hand
        // over an object that implements other interfaces too.
        S_OK
    })
}

/// `IObjectWithSite::GetSite`.
unsafe extern "system" fn site_get_site(
    this: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    contain_hresult(|| unsafe {
        if riid.is_null() || ppv.is_null() {
            return E_POINTER;
        }
        *ppv = ptr::null_mut();
        let interface = &*(this as *const SiteInterface);
        let frame = (*interface.owner).frame;
        if frame.is_null() {
            // No site: `IObjectWithSite` fails this call.
            return E_FAIL;
        }
        // Query outside of any lock: the frame may re-enter this handler.
        query_interface_ptr(frame, riid, ppv)
    })
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

/// Reads the Explorer stream only on the COM apartment that owns it.
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

impl Drop for StreamReader {
    fn drop(&mut self) {
        unsafe { stream_release(self.stream) };
    }
}

/// A raw pointer that is explicitly moved between threads. Only used for a
/// stream produced by `CoMarshalInterThreadInterfaceInStream`, which is
/// designed to be read on another apartment.
struct SendPtr(*mut c_void);

unsafe impl Send for SendPtr {}

/// Marshal the host stream so the session's spool thread can read it without
/// the host apartment ever copying the whole file. Must be called on the
/// apartment that owns `stream`; the result is consumed by
/// [`unmarshal_stream`].
unsafe fn marshal_stream(stream: *mut c_void) -> Result<*mut c_void, String> {
    let mut marshalled: *mut c_void = ptr::null_mut();
    let result = CoMarshalInterThreadInterfaceInStream(&IID_ISTREAM, stream, &mut marshalled);
    if result < 0 || marshalled.is_null() {
        return Err(format!(
            "CoMarshalInterThreadInterfaceInStream failed: 0x{result:08X}"
        ));
    }
    Ok(marshalled)
}

/// A stream unmarshalled onto the calling thread. Dropping it releases the
/// stream and tears down the apartment this call initialized.
struct UnmarshalledStream {
    stream: *mut c_void,
}

// The pointer is only ever created and used on the thread that unmarshalled it
// (`MarshalledReader::read` unmarshals lazily on the spool thread; the reader is
// built with `stream: None` on the host apartment, so no live stream crosses a
// thread boundary).
unsafe impl Send for UnmarshalledStream {}

unsafe fn unmarshal_stream(marshalled: *mut c_void) -> io::Result<UnmarshalledStream> {
    let result = CoInitializeEx(ptr::null_mut(), COINIT_MULTITHREADED);
    if result < 0 {
        // The marshalled stream was never consumed; release it ourselves.
        stream_release(marshalled);
        return Err(io::Error::other(format!(
            "CoInitializeEx failed: 0x{result:08X}"
        )));
    }
    let mut stream: *mut c_void = ptr::null_mut();
    let result = CoGetInterfaceAndReleaseStream(marshalled, &IID_ISTREAM, &mut stream);
    if result < 0 {
        // CoGetInterfaceAndReleaseStream released the marshalled stream.
        CoUninitialize();
        return Err(io::Error::other(format!(
            "CoGetInterfaceAndReleaseStream failed: 0x{result:08X}"
        )));
    }
    Ok(UnmarshalledStream { stream })
}

impl Read for UnmarshalledStream {
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

impl Drop for UnmarshalledStream {
    fn drop(&mut self) {
        unsafe {
            stream_release(self.stream);
            CoUninitialize();
        }
    }
}

/// A host `IStream` marshalled for the session's spool thread. The stream is
/// unmarshalled lazily on the first `read`, which is the thread that will read
/// it, and released when this reader is dropped.
struct MarshalledReader {
    marshalled: SendPtr,
    stream: Option<UnmarshalledStream>,
}

impl MarshalledReader {
    unsafe fn new(marshalled: *mut c_void) -> Self {
        Self {
            marshalled: SendPtr(marshalled),
            stream: None,
        }
    }
}

impl Read for MarshalledReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.stream.is_none() {
            // The marshalled reference is consumed by this attempt: either
            // unmarshal_stream releases it on failure, or COM consumes it on
            // success. Clear the reader's copy first so Drop cannot release
            // the same reference a second time.
            let marshalled = std::mem::replace(&mut self.marshalled.0, ptr::null_mut());
            self.stream = Some(unsafe { unmarshal_stream(marshalled) }?);
        }
        self.stream
            .as_mut()
            .expect("unmarshalled above")
            .read(buffer)
    }
}

impl Drop for MarshalledReader {
    fn drop(&mut self) {
        let marshalled = std::mem::replace(&mut self.marshalled.0, ptr::null_mut());
        if !marshalled.is_null() {
            // Never unmarshalled (for example the session was cancelled before
            // the first read); release the marshalled stream directly.
            unsafe { stream_release(marshalled) }
        }
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
    contain_hresult(|| unsafe {
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
    })
}

unsafe extern "system" fn preview_set_window(this: *mut c_void, parent: HWND) -> HRESULT {
    contain_hresult(|| unsafe {
        let object = &mut *object_from_this(this);
        object.parent = parent;
        if !object.window.is_null() && !parent.is_null() {
            SetParent(object.window, parent);
        }
        S_OK
    })
}

unsafe extern "system" fn preview_set_rect(this: *mut c_void, rect: *const RECT) -> HRESULT {
    contain_hresult(|| unsafe {
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
    })
}

unsafe extern "system" fn preview_set_focus(this: *mut c_void) -> HRESULT {
    contain_hresult(|| unsafe {
        let object = &*object_from_this(this);
        if !object.window.is_null() {
            SetFocus(object.window);
        }
        S_OK
    })
}

unsafe extern "system" fn preview_query_focus(_this: *mut c_void, focus: *mut HWND) -> HRESULT {
    contain_hresult(|| unsafe {
        if focus.is_null() {
            return E_POINTER;
        }
        *focus = GetFocus();
        S_OK
    })
}

unsafe extern "system" fn preview_translate_accelerator(
    this: *mut c_void,
    message: *mut MSG,
) -> HRESULT {
    contain_hresult(|| unsafe {
        if message.is_null() {
            return E_POINTER;
        }
        // The pane consumes nothing through this entry point: its local
        // hotkey arrives as a directly focused WM_KEYDOWN (07A-11 subset 1).
        // Anything the host offers here belongs to the host frame, so an
        // unhandled key is forwarded; fail closed (S_FALSE) without a site,
        // so the shell keeps the key.
        let frame = (*object_from_this(this)).frame;
        if frame.is_null() {
            return S_FALSE;
        }
        let table = *(frame as *mut *const IPreviewHandlerFrameVtbl);
        ((*table).translate_accelerator)(frame, message)
    })
}

unsafe extern "system" fn preview_unload(this: *mut c_void) -> HRESULT {
    contain_hresult(|| unsafe {
        let object = &mut *object_from_this(this);
        object.cleanup();
        S_OK
    })
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
    // Registration is idempotent, but not once-only: the last COM object
    // unregisters the class and a subsequent preview must register it again.
    {
        let class_name = CLASS_NAME.get_or_init(|| wide(WINDOW_CLASS_NAME));
        let atom = register_window_class(this_module(), class_name);
        if atom == 0 {
            let error = GetLastError();
            if error != ERROR_CLASS_ALREADY_EXISTS {
                // Painting still falls back to an empty window; do not panic in
                // the Explorer host.
                eprintln!("csv-preview: RegisterClassW failed: {error}");
            }
        }
    }
}

/// `class_name` must outlive the registration: Windows keeps the pointer.
/// The class belongs to the DLL's module, not to the host EXE, so a DLL
/// reload re-registers cleanly instead of reusing a stale class.
unsafe fn register_window_class(module: HINSTANCE, class_name: &[u16]) -> u16 {
    let window_class = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(preview_window_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: module,
        hIcon: ptr::null_mut(),
        hCursor: ptr::null_mut(),
        hbrBackground: ptr::null_mut(),
        lpszMenuName: ptr::null(),
        lpszClassName: class_name.as_ptr(),
    };
    RegisterClassW(&window_class)
}

/// Remove the class once no object of the library is left (07A-06). A
/// failure is tolerated: Windows refuses to delete a class whose windows are
/// still alive, and such residue is harmless.
unsafe fn unregister_window_class() {
    let Some(class_name) = CLASS_NAME.get() else {
        return;
    };
    UnregisterClassW(class_name.as_ptr(), this_module());
}

/// Hand the rest of the host stream to a session without copying it twice.
///
/// A standard shell stream is marshalled so the session's spool thread reads it
/// directly: one copy, off the host apartment. A stream that cannot be
/// marshalled is read here within the spool cap as a fallback, keeping the
/// previous behaviour for that rare case only.
fn start_spooled_session(
    reader: &mut StreamReader,
    head: Vec<u8>,
    total_bytes: Option<u64>,
    request: &ScanRequest,
    config: PreviewConfig,
) -> Result<PreviewSession, crate::session::PreviewError> {
    match unsafe { marshal_stream(reader.stream) } {
        Ok(marshalled) => PreviewSession::start_with_head(
            head,
            unsafe { MarshalledReader::new(marshalled) },
            total_bytes,
            request,
            config,
        ),
        Err(_) => {
            let rest = crate::session::copy_owned_source(reader, config.max_spool_bytes)
                .map_err(|error| crate::session::PreviewError::Io(error.to_string()))?;
            PreviewSession::start_with_head(head, rest, total_bytes, request, config)
        }
    }
}

unsafe extern "system" fn preview_do_preview(this: *mut c_void) -> HRESULT {
    contain_hresult(|| unsafe {
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

        ensure_window_class();
        let width = (object.rect.right - object.rect.left).max(320);
        let height = (object.rect.bottom - object.rect.top).max(200);
        let class_name = wide(WINDOW_CLASS_NAME);
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
            this_module(),
            object as *mut PreviewObject as *mut c_void,
        );
        if window.is_null() {
            object.set_error_text("CSV preview could not create its child window.");
            return E_FAIL;
        }
        object.window = window;
        SetWindowLongPtrW(window, GWLP_USERDATA, object as *mut PreviewObject as isize);

        let request = match configured_request(ScanRequest::preview(), None) {
            Ok(request) => request,
            Err(error) => {
                object.set_error_text(&format!("Settings could not be loaded: {error}"));
                UpdateWindow(object.window);
                return S_OK;
            }
        };
        let reader = StreamReader::new(object.stream);
        let length_hint = stream_size_hint(object.stream);
        let config = PreviewConfig::default();

        // Read only the bounded head here, on the host's own apartment, so the
        // first screen appears promptly. The rest of the stream is read on the
        // session's spool thread (marshalled across apartments), so this thread
        // is never blocked for a whole-file copy.
        let mut reader = reader;
        let head = match crate::session::read_head(&mut reader, config.head_bytes) {
            Ok(head) => head,
            Err(error) => {
                object.set_error_text(&format!("CSV preview could not read stream: {error}"));
                UpdateWindow(object.window);
                return S_OK;
            }
        };
        let session = match start_spooled_session(&mut reader, head, length_hint, &request, config)
        {
            Ok(session) => Arc::new(session),
            Err(error) => {
                object.set_error_text(&format!("CSV preview could not start: {error}"));
                UpdateWindow(object.window);
                return S_OK;
            }
        };
        object.session = Some(session);
        object.refresh_text();
        ShowWindow(window, SW_SHOW);
        UpdateWindow(window);
        S_OK
    })
}

impl PreviewObject {
    unsafe fn set_error_text(&mut self, message: &str) {
        self.text = wide_text(&format!(
            "CSV viewer and preview — Made by Maria\n\nCould not preview this file.\n{message}\n\nRead-only."
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
        let mut width = 80usize;
        if !self.window.is_null() {
            let dc = GetDC(self.window);
            if !dc.is_null() {
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(self.window, &mut client);
                let old_font = if !self.font.is_null() {
                    SelectObject(dc, self.font)
                } else {
                    ptr::null_mut()
                };
                let mut size = SIZE { cx: 0, cy: 0 };
                let sample = wide("M");
                if GetTextExtentPoint32W(dc, sample.as_ptr(), 1, &mut size) != 0 && size.cx > 0 {
                    width = ((client.right - client.left - 16).max(1) / size.cx).max(1) as usize;
                }
                if !old_font.is_null() {
                    SelectObject(dc, old_font);
                }
                ReleaseDC(self.window, dc);
            }
        }
        let mut text = render_text(&report, width, 38);
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

fn configured_request(
    mut request: ScanRequest,
    path: Option<&std::path::Path>,
) -> Result<ScanRequest, String> {
    let settings = Settings::load_checked(path)?;
    settings.apply_to(&mut request);
    Ok(request)
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
    let request = match configured_request(ScanRequest::full(), None) {
        Ok(request) => request,
        Err(error) => {
            object.set_error_text(&format!("Settings could not be loaded: {error}"));
            return;
        }
    };
    let timeout = Duration::from_millis(request.limits.deadline_ms.unwrap_or(120_000));
    if let Err(error) =
        session.start_full_scan(Arc::new(WorkerBackend::new(worker)), request, timeout)
    {
        object.set_error_text(&format!("Full scan could not start: {error}"));
        return;
    }
    let waiting = session.clone();
    let window_value = window as usize;
    if crate::lifecycle::spawn(
        thread::Builder::new().name("csv-preview-scan-waiter".to_string()),
        move || {
            let _ = waiting.wait_for_scan(timeout + Duration::from_secs(1));
            PostMessageW(window_value as HWND, WM_APP_DONE, 0, 0);
        },
    )
    .is_err()
    {
        // The scan itself keeps running inside the session; only the
        // completion ping would be missing, so the pane keeps the bounded
        // preview instead of aborting the host.
        object.set_error_text("Full scan is running but its completion notice could not start.");
    }
    object.refresh_text();
}

static CLASS_NAME: OnceLock<Vec<u16>> = OnceLock::new();

const WINDOW_CLASS_NAME: &str = "CsvViewerPreview";

/// Font em height for a DPI (07A-10). Mirrors the legacy LOGPIXELSY formula
/// so the lazy WM_PAINT builder and the DPI rebuild agree: a 14 px em at the
/// 96-dpi reference scale, clamped to at least one pixel.
fn font_em_height(dpi: i32) -> i32 {
    -(14 * dpi.max(1) / 96).max(1)
}

/// The window's effective DPI (07A-10). `GetDpiForWindow` is per-monitor
/// aware and also valid for child windows; the legacy device-context metric
/// is the fallback for a window that cannot report one. Both null inputs
/// yield 0 (unknown).
unsafe fn window_dpi(window: HWND, device: HDC) -> i32 {
    let dpi = GetDpiForWindow(window) as i32;
    if dpi > 0 {
        dpi
    } else if !device.is_null() {
        GetDeviceCaps(device, LOGPIXELSY)
    } else {
        0
    }
}

/// Rebuilds the cached preview font for the window's current DPI (07A-10),
/// so a pane carried to a different monitor rescales. A null window keeps
/// the lazy path: WM_PAINT then rebuilds the font from the paint device.
fn rebuild_font_for_dpi(window: HWND, font: HFONT) -> HFONT {
    unsafe {
        if !font.is_null() {
            DeleteObject(font);
        }
        // Without a window DPI, WM_PAINT rebuilds lazily from the device.
        let dpi = window_dpi(window, ptr::null_mut());
        if dpi == 0 {
            return ptr::null_mut();
        }
        CreateFontW(
            font_em_height(dpi),
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
        )
    }
}

unsafe extern "system" fn preview_window_proc(
    window: HWND,
    message: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    contain_lresult(window, message, wparam, lparam, || unsafe {
        let object = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut PreviewObject;
        match message {
            WM_SIZE | WM_DPICHANGED | WM_DPICHANGED_AFTERPARENT => {
                if !object.is_null() {
                    if message != WM_SIZE && !(*object).font.is_null() {
                        // 07A-10: a DPI change (children only ever see the
                        // AFTERPARENT form) rescales the font at the window's
                        // current DPI instead of merely dropping it.
                        (*object).font = rebuild_font_for_dpi(window, (*object).font);
                    }
                    (*object).refresh_text();
                }
                DefWindowProcW(window, message, wparam, lparam)
            }
            WM_PAINT => {
                paint_window(window, object);
                0
            }
            WM_ERASEBKGND => 1,
            WM_KEYDOWN if wparam == VK_F => {
                if !object.is_null() {
                    start_full_scan(&mut *object, window);
                }
                0
            }
            WM_MOUSEACTIVATE => {
                // 07A-11: a click on the pane moves keyboard focus into it.
                if !object.is_null() {
                    SetFocus(window);
                }
                MA_ACTIVATE as LRESULT
            }
            WM_LBUTTONDOWN => {
                if !object.is_null() {
                    SetFocus(window);
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
    })
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
                font_em_height(window_dpi(window, device)),
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
            object.refresh_text();
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
/// # Safety
/// The caller must provide valid COM interface identifiers and output pointer.
pub unsafe extern "system" fn DllGetClassObject(
    clsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    contain_hresult(|| unsafe {
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
    })
}

#[no_mangle]
/// # Safety
/// Must be called according to the COM DLL lifetime contract.
pub unsafe extern "system" fn DllCanUnloadNow() -> HRESULT {
    // A panic here means the counters cannot be trusted: keep the DLL
    // resident (S_FALSE) instead of allowing an unload. A library thread
    // still running is also a refusal: unloading would leave it executing
    // unmapped code.
    contain_hresult_or(S_FALSE, || {
        if crate::lifecycle::live_threads() != 0
            || LIVE_COM_OBJECTS.load(Ordering::SeqCst) != 0
            || SERVER_LOCKS.load(Ordering::SeqCst) != 0
        {
            S_FALSE
        } else {
            S_OK
        }
    })
}

#[no_mangle]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    contain_hresult(|| unsafe {
        let Some(path) = module_path() else {
            return E_FAIL;
        };
        register_module(&path.to_string_lossy(), false)
    })
}

#[no_mangle]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    contain_hresult(unregister_module)
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
    contain_hresult(|| unsafe { query_factory(this as *mut ClassFactory, riid, ppv) })
}

unsafe extern "system" fn factory_add_ref(this: *mut c_void) -> ULONG {
    contain_ulong(|| unsafe {
        let factory = &*(this as *const ClassFactory);
        factory.refcount.fetch_add(1, Ordering::SeqCst) + 1
    })
}

unsafe fn release_factory(factory: *mut ClassFactory) -> ULONG {
    let previous = (*factory).refcount.fetch_sub(1, Ordering::SeqCst);
    if previous == 1 {
        LIVE_COM_OBJECTS.fetch_sub(1, Ordering::SeqCst);
        drop(Box::from_raw(factory));
        if LIVE_COM_OBJECTS.load(Ordering::SeqCst) == 0 {
            unregister_window_class();
        }
        0
    } else {
        previous - 1
    }
}

unsafe extern "system" fn factory_release(this: *mut c_void) -> ULONG {
    contain_ulong(|| unsafe { release_factory(this as *mut ClassFactory) })
}

unsafe extern "system" fn factory_create_instance(
    this: *mut c_void,
    outer: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    contain_hresult(|| unsafe {
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
            site: SiteInterface {
                vtbl: &SITE_VTBL,
                owner: ptr::null_mut(),
            },
            frame: ptr::null_mut(),
        });
        object.init.owner = &mut *object as *mut PreviewObject;
        object.site.owner = &mut *object as *mut PreviewObject;
        LIVE_COM_OBJECTS.fetch_add(1, Ordering::SeqCst);
        let raw = Box::into_raw(object);
        let result = query_preview_interface(raw, riid, ppv);
        object_release(raw);
        result
    })
}

unsafe extern "system" fn factory_lock_server(_this: *mut c_void, lock: BOOL) -> HRESULT {
    contain_hresult(|| {
        if lock != 0 {
            SERVER_LOCKS.fetch_add(1, Ordering::SeqCst);
        } else {
            SERVER_LOCKS.fetch_sub(1, Ordering::SeqCst);
        }
        S_OK
    })
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
    // Preflight over the user hive AND the merged classes root: a
    // machine-wide handler (HKLM, e.g. an office suite) is otherwise
    // silently shadowed by our per-user key.
    match crate::registration::find_foreign_associations(&mut WindowsRegistry, module) {
        Ok(conflicts) if conflicts.is_empty() => {}
        Ok(conflicts) => {
            for conflict in &conflicts {
                eprintln!("csv-preview: {}", conflict.message());
            }
            if !force {
                eprintln!(
                    "csv-preview: registration refused; run with force to replace the foreign handler"
                );
                return ERROR_ALREADY_EXISTS;
            }
        }
        Err(error) => {
            // The preflight cannot prove safety but also found no foreign
            // value; proceed as before and let apply_plan's rollback bound
            // the damage.
            eprintln!("csv-preview: registration preflight failed: {error}");
        }
    }
    let plan = register_plan(module);
    if let Err((error, rollback_errors)) = apply_plan(&mut WindowsRegistry, &plan) {
        eprintln!(
            "csv-preview: registration failed: {error}; rollback errors: {rollback_errors:?}"
        );
        return E_FAIL;
    }
    S_OK
}

fn unregister_module() -> HRESULT {
    let module = match unsafe { module_path() } {
        Some(path) => path.to_string_lossy().into_owned(),
        None => return E_FAIL,
    };
    match unregister(&mut WindowsRegistry, &module) {
        Ok(()) => S_OK,
        Err(error) => {
            eprintln!("csv-preview: unregistration failed: {error}");
            E_FAIL
        }
    }
}

/// CLI entry for `csv-viewer.exe --register` (cfg(windows) only). The package
/// is registered from the running executable's own directory; without a DLL
/// next to it there is nothing to register.
fn resolve_module_dll(module: &std::path::Path) -> Option<PathBuf> {
    let dll = module.parent()?.join("csv_preview.dll");
    dll.is_file().then_some(dll)
}

pub fn register_command(force: bool) -> HRESULT {
    match unsafe { module_path() }.and_then(|path| resolve_module_dll(&path)) {
        Some(path) => register_module(&path.to_string_lossy(), force),
        None => {
            eprintln!(
                "csv-preview: this DLL was not found through the running executable; \
                 --register only works for modules on disk"
            );
            E_FAIL
        }
    }
}

/// CLI entry for `csv-viewer.exe --unregister` (cfg(windows) only).
pub fn unregister_command() -> HRESULT {
    let Some(module) = (unsafe { module_path() }).and_then(|path| resolve_module_dll(&path)) else {
        eprintln!("csv-preview: csv_preview.dll was not found next to the executable");
        return E_FAIL;
    };
    match unregister(&mut WindowsRegistry, &module.to_string_lossy()) {
        Ok(()) => S_OK,
        Err(error) => {
            eprintln!("csv-preview: unregistration failed: {error}");
            E_FAIL
        }
    }
}

struct WindowsRegistry;

impl Registry for WindowsRegistry {
    type Error = String;
    fn read(&mut self, key: &str, name: Option<&str>) -> Result<Option<Value>, String> {
        unsafe { read_value_under(HKEY_CURRENT_USER, &registry_path(key), name) }
    }
    fn read_classes_root(
        &mut self,
        key: &str,
        name: Option<&str>,
    ) -> Result<Option<Value>, String> {
        // HKEY_CLASSES_ROOT is the HKLM∪HKCU merge; writes keep going to the
        // user hive, this is only for the preflight.
        unsafe { read_value_under(HKEY_CLASSES_ROOT, key, name) }
    }
    fn delete_value(&mut self, key: &str, name: Option<&str>) -> Result<(), String> {
        unsafe { delete_value(key, name) }
    }
    fn write(&mut self, entry: &RegistryValue) -> Result<(), String> {
        unsafe { write_value(entry) }
    }
    fn delete_tree(&mut self, key: &str) -> Result<(), String> {
        let full = wide(&registry_path(key));
        let status = unsafe { SHDeleteKeyW(HKEY_CURRENT_USER, full.as_ptr()) };
        // A tree that was never created is already as removed as it can be.
        if status == 0 || status == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(format!("SHDeleteKeyW failed with {status}"))
        }
    }
}

fn registry_path(key: &str) -> String {
    if key == PREVIEW_HANDLERS_KEY {
        key.to_owned()
    } else {
        format!("Software\\Classes\\{key}")
    }
}

unsafe fn write_value(entry: &RegistryValue) -> Result<(), String> {
    let full = registry_path(&entry.key);
    let handle = create_key(&full)?;
    let name = entry.name.map(wide);
    let name_ptr = name
        .as_ref()
        .map(|name| name.as_ptr())
        .unwrap_or(ptr::null());
    let (value_type, bytes): (u32, Vec<u8>) = match &entry.value {
        Value::Text(text) => {
            let units = wide(text);
            (REG_SZ, units.iter().flat_map(|u| u.to_le_bytes()).collect())
        }
        Value::Dword(number) => (REG_DWORD, number.to_le_bytes().to_vec()),
    };
    let status = RegSetValueExW(
        handle,
        name_ptr,
        0,
        value_type,
        bytes.as_ptr(),
        bytes.len() as u32,
    );
    RegCloseKey(handle);
    if status == 0 {
        Ok(())
    } else {
        Err(format!("RegSetValueExW failed with {status}"))
    }
}

unsafe fn delete_value(key: &str, name: Option<&str>) -> Result<(), String> {
    let full = wide(&registry_path(key));
    let mut handle: HKEY = ptr::null_mut();
    let status = RegOpenKeyExW(HKEY_CURRENT_USER, full.as_ptr(), 0, KEY_WRITE, &mut handle);
    if status == 2 {
        return Ok(());
    }
    if status != 0 {
        return Err(format!("RegOpenKeyExW failed with {status}"));
    }
    let name = name.map(wide);
    let status = RegDeleteValueW(handle, name.as_ref().map_or(ptr::null(), |n| n.as_ptr()));
    RegCloseKey(handle);
    if status == 0 || status == 2 {
        Ok(())
    } else {
        Err(format!("RegDeleteValueW failed with {status}"))
    }
}

unsafe fn read_value_under(
    root: HKEY,
    key: &str,
    name: Option<&str>,
) -> Result<Option<Value>, String> {
    let full = wide(key);
    let mut handle: HKEY = ptr::null_mut();
    let status = RegOpenKeyExW(root, full.as_ptr(), 0, KEY_READ, &mut handle);
    if status == 2 {
        return Ok(None);
    }
    if status != 0 {
        return Err(format!("RegOpenKeyExW failed with {status}"));
    }
    let name = name.map(wide);
    let name_ptr = name.as_ref().map_or(ptr::null(), |n| n.as_ptr());
    let mut value_type = 0;
    let mut length = 0;
    let status = RegQueryValueExW(
        handle,
        name_ptr,
        ptr::null_mut(),
        &mut value_type,
        ptr::null_mut(),
        &mut length,
    );
    if status != 0 {
        RegCloseKey(handle);
        return if status == 2 {
            Ok(None)
        } else {
            Err(format!("RegQueryValueExW failed with {status}"))
        };
    }
    if value_type != REG_SZ && value_type != REG_DWORD {
        RegCloseKey(handle);
        return Ok(None);
    }
    let mut buffer = vec![0u8; length as usize];
    let status = RegQueryValueExW(
        handle,
        name_ptr,
        ptr::null_mut(),
        &mut value_type,
        buffer.as_mut_ptr(),
        &mut length,
    );
    RegCloseKey(handle);
    if status == 2 {
        return Ok(None);
    }
    if status != 0 {
        return Err(format!("RegQueryValueExW failed with {status}"));
    }
    buffer.truncate(length as usize);
    match value_type {
        REG_DWORD => {
            let mut bytes = [0u8; 4];
            for (slot, byte) in bytes.iter_mut().zip(buffer.iter()) {
                *slot = *byte;
            }
            Ok(Some(Value::Dword(u32::from_le_bytes(bytes))))
        }
        REG_SZ => {
            let utf16: Vec<u16> = buffer
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .take_while(|unit| *unit != 0)
                .collect();
            Ok(Some(Value::Text(String::from_utf16_lossy(&utf16))))
        }
        _ => Ok(None),
    }
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

#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(reserved: *mut c_void, co_init: u32) -> HRESULT;
    fn CoUninitialize();
    fn CoMarshalInterThreadInterfaceInStream(
        riid: *const GUID,
        object: *mut c_void,
        stream: *mut *mut c_void,
    ) -> HRESULT;
    fn CoGetInterfaceAndReleaseStream(
        stream: *mut c_void,
        riid: *const GUID,
        object: *mut *mut c_void,
    ) -> HRESULT;
}

#[link(name = "user32")]
extern "system" {
    fn GetDC(window: HWND) -> HDC;
    fn ReleaseDC(window: HWND, dc: HDC) -> i32;
    fn RegisterClassW(class: *const WNDCLASSW) -> u16;
    fn UnregisterClassW(class: LPCWSTR, instance: HINSTANCE) -> BOOL;
    #[cfg(test)]
    fn GetClassInfoExW(instance: HINSTANCE, class: LPCWSTR, info: *mut WNDCLASSEXW) -> BOOL;
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
    /// Per-monitor DPI of the window; children included (07A-10).
    fn GetDpiForWindow(window: HWND) -> UINT;
    #[cfg(test)]
    fn SendMessageW(window: HWND, message: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT;
}

#[link(name = "gdi32")]
extern "system" {
    fn GetDeviceCaps(dc: HDC, index: i32) -> i32;
    fn GetTextExtentPoint32W(dc: HDC, text: LPCWSTR, count: i32, size: *mut SIZE) -> BOOL;
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

#[link(name = "shlwapi")]
extern "system" {
    /// Deletes a key and everything below it (the C1 uninstall policy).
    fn SHDeleteKeyW(root: HKEY, sub_key: LPCWSTR) -> LONG;
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
    use std::sync::atomic::AtomicBool;

    const COINIT_APARTMENTTHREADED: u32 = 0x2;

    #[link(name = "user32")]
    extern "system" {
        fn PeekMessageW(msg: *mut MSG, hwnd: HWND, min: u32, max: u32, remove: u32) -> BOOL;
        fn TranslateMessage(msg: *const MSG) -> BOOL;
        fn DispatchMessageW(msg: *const MSG) -> LRESULT;
    }

    #[cfg(test)]
    unsafe fn font_average_char_width(window: HWND, font: HFONT) -> i32 {
        let dc = GetDC(window);
        let previous = SelectObject(dc, font);
        let mut size = SIZE { cx: 0, cy: 0 };
        let sample = wide("M");
        let measured = GetTextExtentPoint32W(dc, sample.as_ptr(), 1, &mut size);
        SelectObject(dc, previous);
        ReleaseDC(window, dc);
        assert_ne!(measured, 0, "text metrics must be measurable");
        size.cx
    }

    /// 07A-11 subset 1: a click must focus the pane (its F hotkey is then
    /// reachable), WM_KEYDOWN must compare VK codes (0x46 = VK_F triggers,
    /// and the old lowercase-literal branch, 0x66, spuriously matched
    /// VK_NUMPAD6). Drives the real window proc through real messages.
    #[test]
    fn preview_and_full_scan_refuse_invalid_user_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, b"invalid JSON").unwrap();
        assert!(configured_request(ScanRequest::preview(), Some(&path)).is_err());
        assert!(configured_request(ScanRequest::full(), Some(&path)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"invalid JSON");
        std::fs::write(&path, b"{}").unwrap();
        assert!(configured_request(ScanRequest::preview(), Some(&path)).is_ok());
    }

    #[test]
    fn click_focuses_the_pane_and_vks_compare_correctly() {
        unsafe {
            let instance = GetModuleHandleW(ptr::null());
            let parent_class = wide("CsvViewerPreviewTestParentClass");
            let atom = RegisterClassW(&WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(DefWindowProcW),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance,
                hIcon: ptr::null_mut(),
                hCursor: ptr::null_mut(),
                hbrBackground: ptr::null_mut(),
                lpszMenuName: ptr::null(),
                lpszClassName: parent_class.as_ptr(),
            });
            assert_ne!(
                atom,
                0,
                "parent RegisterClassW failed with {}",
                GetLastError()
            );
            let parent = CreateWindowExW(
                0,
                parent_class.as_ptr(),
                wide("parent").as_ptr(),
                0,
                0,
                0,
                400,
                300,
                ptr::null_mut(),
                ptr::null_mut(),
                instance,
                ptr::null_mut(),
            );
            assert!(!parent.is_null(), "parent CreateWindowExW failed");
            let class_name = wide("CsvViewerPreviewFocusTestClass");
            let atom = register_window_class(instance, &class_name);
            assert_ne!(
                atom,
                0,
                "pane RegisterClassW failed with {}",
                GetLastError()
            );
            let pane = CreateWindowExW(
                0,
                class_name.as_ptr(),
                wide("pane").as_ptr(),
                WS_CHILD,
                0,
                0,
                120,
                60,
                parent,
                ptr::null_mut(),
                instance,
                ptr::null_mut(),
            );
            assert!(!pane.is_null(), "pane CreateWindowExW failed");
            let object = Box::into_raw(Box::new(PreviewObject {
                vtbl: &PREVIEW_VTBL,
                refcount: AtomicU32::new(1),
                stream: ptr::null_mut(),
                parent,
                rect: RECT {
                    left: 0,
                    top: 0,
                    right: 120,
                    bottom: 60,
                },
                window: pane,
                font: ptr::null_mut(),
                brush: ptr::null_mut(),
                session: None,
                text: Vec::new(),
                init: InitInterface {
                    vtbl: &INIT_VTBL,
                    owner: ptr::null_mut(),
                },
                site: SiteInterface {
                    vtbl: &SITE_VTBL,
                    owner: ptr::null_mut(),
                },
                frame: ptr::null_mut(),
            }));
            SetWindowLongPtrW(pane, GWLP_USERDATA, object as isize);

            // A click activates the child pane (07A-11: set focus to it).
            assert_ne!(GetFocus(), pane, "precondition: focus elsewhere");
            SendMessageW(pane, WM_MOUSEACTIVATE, parent as WPARAM, 0);
            assert_eq!(GetFocus(), pane, "clicking the pane must focus it");

            // VK_F (0x46) must reach the scan path; with no session the scan
            // fails into the error text, which proves the arm ran.
            SendMessageW(pane, WM_KEYDOWN, VK_F, 0);
            assert!(
                !(*object).text.is_empty(),
                "VK_F must trigger the scan path"
            );

            // The old b'f' comparison (0x66) spuriously matched VK_NUMPAD6;
            // it must not trigger anything.
            (*object).text.clear();
            SendMessageW(pane, WM_KEYDOWN, 0x66, 0);
            assert!(
                (*object).text.is_empty(),
                "VK_NUMPAD6 must not trigger the scan any more"
            );

            SetWindowLongPtrW(pane, GWLP_USERDATA, 0);
            DestroyWindow(pane);
            DestroyWindow(parent);
            drop(Box::from_raw(object));
        }
    }

    /// A minimal host `IPreviewHandlerFrame` that records the keys it is
    /// asked to translate, mirroring parquet-viewer L5's harness (07A-11).
    #[allow(dead_code)]
    struct FakeFrame {
        vtbl: *const IPreviewHandlerFrameVtbl,
        refs: AtomicU32,
        translated: AtomicBool,
        last_key: AtomicUsize,
    }

    unsafe extern "system" fn fake_frame_qi(
        this: *mut c_void,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if iid.is_null() || out.is_null() {
            return E_POINTER;
        }
        *out = ptr::null_mut();
        let iid = *iid;
        if iid == IID_IUNKNOWN || iid == IID_IPREVIEWHANDLERFRAME {
            (*(this as *mut FakeFrame))
                .refs
                .fetch_add(1, Ordering::SeqCst);
            *out = this;
            S_OK
        } else {
            E_NOINTERFACE
        }
    }

    unsafe extern "system" fn fake_frame_add_ref(this: *mut c_void) -> ULONG {
        let frame = this as *mut FakeFrame;
        (*frame).refs.fetch_add(1, Ordering::SeqCst) + 1
    }

    unsafe extern "system" fn fake_frame_release(this: *mut c_void) -> ULONG {
        let frame = this as *mut FakeFrame;
        let previous = (*frame).refs.fetch_sub(1, Ordering::SeqCst);
        if previous == 1 {
            drop(Box::from_raw(frame));
            0
        } else {
            previous - 1
        }
    }

    unsafe extern "system" fn fake_frame_get_window_context(
        _this: *mut c_void,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if out.is_null() {
            return E_POINTER;
        }
        *out = ptr::null_mut();
        E_FAIL
    }

    unsafe extern "system" fn fake_frame_translate_accelerator(
        this: *mut c_void,
        message: *mut MSG,
    ) -> HRESULT {
        let frame = this as *mut FakeFrame;
        if !message.is_null() {
            (*frame).last_key.store((*message).wParam, Ordering::SeqCst);
            (*frame).translated.store(true, Ordering::SeqCst);
        }
        S_OK
    }

    static FAKE_FRAME_VTBL: IPreviewHandlerFrameVtbl = IPreviewHandlerFrameVtbl {
        query_interface: fake_frame_qi,
        add_ref: fake_frame_add_ref,
        release: fake_frame_release,
        get_window_context: fake_frame_get_window_context,
        translate_accelerator: fake_frame_translate_accelerator,
    };

    /// 07A-11 subset 2: `IObjectWithSite` hands the handler the host site,
    /// from which we query `IPreviewHandlerFrame`; unhandled keys offered
    /// through `TranslateAccelerator` are then forwarded to the frame.
    /// Without a site the handler fails closed (S_FALSE).
    #[test]
    fn unhandled_keys_forward_to_the_host_frame() {
        unsafe {
            let object = Box::new(PreviewObject {
                vtbl: &PREVIEW_VTBL,
                refcount: AtomicU32::new(1),
                stream: ptr::null_mut(),
                parent: ptr::null_mut(),
                rect: RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
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
                site: SiteInterface {
                    vtbl: &SITE_VTBL,
                    owner: ptr::null_mut(),
                },
                frame: ptr::null_mut(),
            });
            let object = Box::into_raw(object);
            (*object).init.owner = object;
            (*object).site.owner = object;

            // Without a site, TranslateAccelerator fails closed and
            // GetSite fails.
            let msg = MSG {
                hwnd: ptr::null_mut(),
                message: WM_KEYDOWN,
                wParam: 0x09, // VK_TAB belongs to the host
                lParam: 0,
                time: 0,
                pt: POINT { x: 0, y: 0 },
            };
            assert_eq!(
                preview_translate_accelerator(object as *mut c_void, &msg as *const _ as *mut MSG),
                S_FALSE
            );
            let mut site: *mut c_void = ptr::null_mut();
            assert_eq!(
                query_preview_interface(object, &IID_IOBJECTWITHSITE, &mut site),
                S_OK
            );
            let site_table = *(site as *mut *const IObjectWithSiteVtbl);
            let mut got: *mut c_void = ptr::null_mut();
            assert_eq!(
                ((*site_table).get_site)(site, &IID_IUNKNOWN, &mut got),
                E_FAIL,
                "GetSite without a site must fail"
            );

            // Attach a fake host frame through SetSite.
            let frame = Box::into_raw(Box::new(FakeFrame {
                vtbl: &FAKE_FRAME_VTBL,
                refs: AtomicU32::new(1),
                translated: AtomicBool::new(false),
                last_key: AtomicUsize::new(0),
            }));
            assert_eq!(((*site_table).set_site)(site, frame as *mut c_void), S_OK);
            assert!(!(*object).frame.is_null(), "SetSite must store the frame");

            // GetSite must hand back a frame reference through its vtable.
            assert_eq!(
                ((*site_table).get_site)(site, &IID_IUNKNOWN, &mut got),
                S_OK
            );
            assert_eq!(
                got, frame as *mut c_void,
                "GetSite must return the stored frame"
            );
            release_interface(got);

            // An unhandled key (Tab) must be forwarded to the frame.
            assert_eq!(
                preview_translate_accelerator(object as *mut c_void, &msg as *const _ as *mut MSG),
                S_OK
            );
            assert!((*frame).translated.load(Ordering::SeqCst));
            assert_eq!((*frame).last_key.load(Ordering::SeqCst), 0x09);

            // SetSite(NULL) tears the site down: the frame reference is
            // released and forwarding fails closed again.
            assert_eq!(((*site_table).set_site)(site, ptr::null_mut()), S_OK);
            assert!((*object).frame.is_null());
            assert_eq!(
                preview_translate_accelerator(object as *mut c_void, &msg as *const _ as *mut MSG),
                S_FALSE
            );
            assert_eq!(
                (*frame).refs.load(Ordering::SeqCst),
                1,
                "the frame reference stored in the handled object must be released with the site"
            );

            // Teardown: the object is dropped directly (release bookkeeping
            // through the COM counters is exercised by the factory path), and
            // the frame's creation reference goes with the test. The stored
            // frame reference was already released by SetSite(NULL), hence
            // the remaining count of 1.
            drop(Box::from_raw(object));
            release_interface(frame as *mut c_void);
        }
    }

    #[test]
    fn fn_font_em_height_mirrors_the_legacy_formula() {
        assert_eq!(font_em_height(96), -14);
        assert_eq!(font_em_height(192), -28);
        // Degenerate DPI still yields at least a one-pixel font.
        assert_eq!(font_em_height(0), -1);
        assert_eq!(font_em_height(1), -1);
    }

    /// 07A-10: a preview pane is a WS_CHILD window, so it receives
    /// WM_DPICHANGED_AFTERPARENT (0x02E3), never WM_DPICHANGED, when the
    /// host Explorer moves between monitors. The delivered message must
    /// rebuild the pane's font at `GetDpiForWindow` scaling. Before the fix
    /// no arm matched 0x02E3 and the font stayed stale.
    #[test]
    fn dpichanged_afterparent_rebuilds_the_font() {
        unsafe {
            let class_name = wide("CsvViewerPreviewDpiTestClass");
            let instance = GetModuleHandleW(ptr::null());
            let atom = register_window_class(instance, &class_name);
            assert_ne!(atom, 0, "RegisterClassW failed with {}", GetLastError());
            let window = CreateWindowExW(
                0,
                class_name.as_ptr(),
                wide("dpi").as_ptr(),
                0,
                0,
                0,
                120,
                60,
                ptr::null_mut(),
                ptr::null_mut(),
                instance,
                ptr::null_mut(),
            );
            assert!(!window.is_null(), "CreateWindowExW failed");
            let stale_font = CreateFontW(
                16,
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
            assert!(!stale_font.is_null());
            let object = Box::into_raw(Box::new(PreviewObject {
                vtbl: &PREVIEW_VTBL,
                refcount: AtomicU32::new(1),
                stream: ptr::null_mut(),
                parent: ptr::null_mut(),
                rect: RECT {
                    left: 0,
                    top: 0,
                    right: 120,
                    bottom: 60,
                },
                window,
                font: stale_font,
                brush: ptr::null_mut(),
                session: None,
                text: Vec::new(),
                init: InitInterface {
                    vtbl: &INIT_VTBL,
                    owner: ptr::null_mut(),
                },
                site: SiteInterface {
                    vtbl: &SITE_VTBL,
                    owner: ptr::null_mut(),
                },
                frame: ptr::null_mut(),
            }));
            SetWindowLongPtrW(window, GWLP_USERDATA, object as isize);

            // The message a child actually receives must rebuild the font.
            SendMessageW(window, WM_DPICHANGED_AFTERPARENT, 0, 0);
            let rebuilt = (*object).font;
            assert!(
                !rebuilt.is_null(),
                "WM_DPICHANGED_AFTERPARENT must leave the font rebuilt"
            );
            let dpi = GetDpiForWindow(window) as i32;
            assert!(dpi > 0, "GetDpiForWindow must report the pane's DPI");
            let expected = CreateFontW(
                font_em_height(dpi),
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
            assert_eq!(
                font_average_char_width(window, rebuilt),
                font_average_char_width(window, expected),
                "the rebuilt font must be sized from GetDpiForWindow"
            );
            assert_ne!(
                rebuilt, stale_font,
                "the stale font must have been dropped and rebuilt"
            );
            assert_ne!(
                font_average_char_width(window, rebuilt),
                font_average_char_width(window, stale_font),
                "the rebuild must actually rescale (stale 16 px vs {dpi} dpi)"
            );

            // The top-level WM_DPICHANGED arm keeps its behavior.
            SendMessageW(window, WM_DPICHANGED, 0, 0);
            assert!(
                !(*object).font.is_null(),
                "WM_DPICHANGED must still rebuild the font"
            );

            SetWindowLongPtrW(window, GWLP_USERDATA, 0);
            DestroyWindow(window);
            drop(Box::from_raw(object));
        }
    }

    /// 07A-06: the class is registered with the module we hand to
    /// `register_window_class`, not with whatever `GetModuleHandleW(NULL)`
    /// resolves. A different always-loaded system DLL proves the point.
    #[test]
    fn window_class_registers_with_the_given_module() {
        let name = wide("CsvViewerPreviewTestClass");
        let exe = unsafe { GetModuleHandleW(ptr::null()) };
        let other = unsafe { GetModuleHandleW(wide("kernel32.dll").as_ptr()) };
        assert!(!other.is_null(), "kernel32 must be loaded");
        assert_ne!(other, exe, "the test needs a non-EXE module handle");
        let atom = unsafe { register_window_class(other, &name) };
        assert_ne!(atom, 0, "RegisterClassW failed with {}", unsafe {
            GetLastError()
        });
        let mut info: WNDCLASSEXW = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
        let found = unsafe { GetClassInfoExW(other, name.as_ptr(), &mut info) };
        assert_ne!(found, 0, "GetClassInfoExW could not find the class");
        assert_eq!(info.hInstance, other, "the class must use the given module");
    }

    /// 07A-06: `DllCanUnloadNow` refuses while any library thread is running;
    /// a thread outliving an otherwise-idle library must not allow an unload.
    #[test]
    fn unload_is_refused_while_a_library_thread_is_alive() {
        let alive = crate::lifecycle::LiveThread::enter();
        assert_eq!(unsafe { DllCanUnloadNow() }, S_FALSE);
        drop(alive);
    }

    /// 07A-05: an unwind out of an `extern "system"` function aborts the
    /// process since Rust 1.81, and prevhost.exe may host other handlers too.
    /// Every contained entry must come back as its fallback value instead;
    /// this test reaching its assertions past four injected panics is the
    /// abort-proof evidence.
    #[test]
    fn panic_in_preview_does_not_abort_host() {
        assert_eq!(contain_hresult(|| panic!("injected preview panic")), E_FAIL);
        assert_eq!(
            contain_hresult_or(S_FALSE, || panic!("injected fallback choice")),
            S_FALSE
        );
        // A refcount fallback of 1 is leak-safe: the DLL simply stays loaded.
        assert_eq!(contain_ulong(|| panic!("injected refcount panic")), 1);
        // The window proc falls back to the default procedure (WM_NULL).
        contain_lresult(ptr::null_mut(), 0, 0, 0, || {
            panic!("injected wndproc panic")
        });
    }

    /// Early IStream failure must leave a diagnostic child pane, including
    /// after the last object/window class was released and used again.
    #[test]
    fn stream_read_failure_creates_a_diagnostic_pane_after_reopen() {
        unsafe extern "system" fn retain(_this: *mut c_void) -> u32 {
            1
        }
        unsafe extern "system" fn fail_read(
            _this: *mut c_void,
            _buffer: *mut c_void,
            _size: u32,
            read: *mut u32,
        ) -> HRESULT {
            *read = 0;
            STG_E_ACCESSDENIED
        }
        unsafe extern "system" fn fail_stat(
            _this: *mut c_void,
            _stat: *mut STATSTG,
            _flags: u32,
        ) -> HRESULT {
            E_FAIL
        }
        let table = IStreamVtbl {
            query_interface: ptr::null(),
            add_ref: retain as *const c_void,
            release: retain as *const c_void,
            read: fail_read,
            write: ptr::null(),
            seek: ptr::null(),
            set_size: ptr::null(),
            copy_to: ptr::null(),
            commit: ptr::null(),
            revert: ptr::null(),
            lock_region: ptr::null(),
            unlock_region: ptr::null(),
            stat: fail_stat,
            clone: ptr::null(),
        };
        let mut stream_table = &table as *const IStreamVtbl;
        for _ in 0..2 {
            unsafe {
                let parent = CreateWindowExW(
                    0,
                    wide("STATIC").as_ptr(),
                    wide("failure test").as_ptr(),
                    0,
                    0,
                    0,
                    640,
                    480,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    this_module(),
                    ptr::null_mut(),
                );
                assert!(!parent.is_null());
                let mut object = Box::new(PreviewObject {
                    vtbl: &PREVIEW_VTBL,
                    refcount: AtomicU32::new(1),
                    stream: &mut stream_table as *mut _ as *mut c_void,
                    parent,
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
                    site: SiteInterface {
                        vtbl: &SITE_VTBL,
                        owner: ptr::null_mut(),
                    },
                    frame: ptr::null_mut(),
                });
                let status = preview_do_preview(&mut *object as *mut _ as *mut c_void);
                let has_window = !object.window.is_null();
                let text = String::from_utf16_lossy(&object.text);
                drop(object);
                DestroyWindow(parent);
                unregister_window_class();
                assert_eq!(status, S_OK, "the diagnostic pane handles the failure");
                assert!(has_window, "failure must not leave the host blank");
                assert!(text.contains("Could not preview this file."), "{text}");
                assert!(text.contains("IStream::Read failed"), "{text}");
                assert!(text.contains("Read-only."), "{text}");
            }
        }
    }

    #[test]
    fn registration_resolves_only_an_existing_companion_dll() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("csv-viewer.exe");
        std::fs::write(&exe, b"test executable").unwrap();
        assert_eq!(resolve_module_dll(&exe), None);
        let dll = dir.path().join("csv_preview.dll");
        std::fs::write(&dll, b"test DLL").unwrap();
        assert_eq!(resolve_module_dll(&exe), Some(dll.clone()));
        assert_eq!(resolve_module_dll(&dll), Some(dll));
    }

    #[test]
    fn parses_the_registration_clsid() {
        let guid = parse_guid(CLSID_CSV_PREVIEW).unwrap();
        assert_eq!(guid.data1, 0x8f6a0f5e);
        assert_eq!(guid.data4[7], 0x60);
    }

    #[link(name = "shlwapi")]
    extern "system" {
        fn SHCreateMemStream(init: *const u8, size: u32) -> *mut c_void;
    }

    /// The marshalling contract the fixed preview path relies on: a host stream
    /// created on the host apartment is readable on another thread after
    /// marshal/unmarshal, and the marshalled stream is consumed exactly once.
    #[test]
    fn a_host_stream_marshals_across_apartments() {
        use std::io::Read;
        let payload: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
        let stream = unsafe { SHCreateMemStream(payload.as_ptr(), payload.len() as u32) };
        assert!(!stream.is_null(), "SHCreateMemStream failed");
        let initialised = unsafe { CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED) };
        assert!(
            initialised >= 0,
            "CoInitializeEx failed: 0x{initialised:08X}"
        );
        let marshalled = unsafe { marshal_stream(stream) }.expect("marshal the host stream");
        let pointer = SendPtr(marshalled);

        let handle = std::thread::spawn(move || {
            let mut reader = MarshalledReader {
                marshalled: pointer,
                stream: None,
            };
            let mut out = Vec::new();
            reader
                .read_to_end(&mut out)
                .expect("read the marshalled stream");
            out
        });
        // A host STA must pump COM calls while its worker reads a proxy.
        // Blocking this apartment in join() deadlocks apartment-bound streams.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while !handle.is_finished() {
            unsafe {
                let mut message: MSG = std::mem::zeroed();
                while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, 1) != 0 {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            assert!(
                std::time::Instant::now() < deadline,
                "STA stream transfer timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let out = handle.join().unwrap();
        assert_eq!(out, payload);
        unsafe {
            stream_release(stream);
            CoUninitialize();
        }
    }
}
