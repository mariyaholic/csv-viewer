// SPDX-License-Identifier: Apache-2.0
// Native integration smoke host. Build with MinGW; run with GUI exe and fixture.
// This is not a substitute for native Windows Narrator / monitor acceptance.
#ifndef UNICODE
#define UNICODE
#endif
#define _UNICODE
#include <windows.h>
#include <commctrl.h>
#include <stdio.h>
#include <string.h>
#include <wchar.h>
#include <wctype.h>
static int fail(const char *what){fprintf(stderr,"FAIL: %s (Win32 %lu)\n",what,GetLastError());return 1;}
// SetWindowText cannot edit another process's controls on Windows. Wine is
// more permissive; use system-marshalled messages and verify the actual content.
static int set_query(HWND h,const wchar_t *value){
 HWND edit=GetDlgItem(h,111);wchar_t text[1025]={0};
 if(!SendMessageW(edit,WM_SETTEXT,0,(LPARAM)value))return 0;
 SendMessageW(edit,WM_GETTEXT,1025,(LPARAM)text);return wcscmp(text,value)==0;
}
static HWND wait_window(const wchar_t *klass,DWORD pid){
 for(int i=0;i<400;i++){HWND h=FindWindowW(klass,0);DWORD owner=0;if(h)GetWindowThreadProcessId(h,&owner);if(owner==pid&&IsWindowVisible(h)&&GetDlgItem(h,117))return h;Sleep(20);}return 0;
}
static int close_gui(HWND h,PROCESS_INFORMATION *pi){
 PostMessageW(h,WM_CLOSE,0,0);if(WaitForSingleObject(pi->hProcess,5000)!=WAIT_OBJECT_0){TerminateProcess(pi->hProcess,9);return 0;}
 DWORD code=1;GetExitCodeProcess(pi->hProcess,&code);CloseHandle(pi->hThread);CloseHandle(pi->hProcess);return code==0;
}
static HWND launch_gui(wchar_t *cmd,const wchar_t *klass,PROCESS_INFORMATION *pi){
 STARTUPINFOW si={0};si.cb=sizeof(si);ZeroMemory(pi,sizeof(*pi));
 if(!CreateProcessW(0,cmd,0,0,FALSE,0,0,0,&si,pi))return 0;
 HWND h=wait_window(klass,pi->dwProcessId);if(!h){TerminateProcess(pi->hProcess,9);CloseHandle(pi->hThread);CloseHandle(pi->hProcess);}return h;
}
typedef struct { DWORD pid; HWND dialog; int credit; } DialogProbe;
static BOOL CALLBACK find_dialog(HWND h,LPARAM param){
 DialogProbe *probe=(DialogProbe*)param;DWORD pid=0;wchar_t name[64];GetWindowThreadProcessId(h,&pid);GetClassNameW(h,name,64);
 if(pid==probe->pid&&!wcscmp(name,L"#32770")&&IsWindowVisible(h)){probe->dialog=h;}
 return TRUE;
}
static BOOL CALLBACK find_credit(HWND h,LPARAM param){
 DialogProbe *probe=(DialogProbe*)param;wchar_t text[4096]={0};SendMessageW(h,WM_GETTEXT,4096,(LPARAM)text);if(wcsstr(text,L"Made by Maria"))probe->credit=1;return TRUE;
}
static int dialog_check(HWND h,DWORD pid,int command,int dismiss,int credit){
 DialogProbe probe={pid,0,0};
 if(command==105)PostMessageW(h,WM_KEYDOWN,VK_F1,0);else PostMessageW(h,WM_COMMAND,command,0);
 for(int i=0;i<300&&!probe.dialog;i++){EnumWindows(find_dialog,(LPARAM)&probe);Sleep(20);}if(!probe.dialog)return 0;
 if(credit)EnumChildWindows(probe.dialog,find_credit,(LPARAM)&probe);
 PostMessageW(probe.dialog,WM_COMMAND,dismiss,0);for(int i=0;i<300&&IsWindow(probe.dialog);i++)Sleep(20);
 return !IsWindow(probe.dialog)&&(!credit||probe.credit)&&IsWindow(h);
}
static int caption_matches_heading(HWND h,const wchar_t *kind){
 wchar_t title[4096]={0},heading[4096]={0};GetWindowTextW(h,title,4096);SendMessageW(GetDlgItem(h,107),WM_GETTEXT,4096,(LPARAM)heading);
 return heading[0]&&wcsstr(title,kind)&&!wcscmp(title,heading);
}
static int ready(HWND h){for(int i=0;i<1000;i++){if(IsWindowEnabled(GetDlgItem(h,102)))return 1;Sleep(20);}return 0;}
static int source_row(HWND h,DWORD pid,const wchar_t *wanted){
 HWND table=GetDlgItem(h,109);LVITEMW row={0};row.state=LVIS_SELECTED|LVIS_FOCUSED;row.stateMask=row.state;
 HANDLE p=OpenProcess(PROCESS_VM_OPERATION|PROCESS_VM_WRITE,FALSE,pid);if(!p)return 0;
 void *remote=VirtualAllocEx(p,0,sizeof(row),MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE);if(!remote){CloseHandle(p);return 0;}
 int ok=WriteProcessMemory(p,remote,&row,sizeof(row),0)!=0;
 if(ok)SendMessageW(table,LVM_SETITEMSTATE,0,(LPARAM)remote);
 VirtualFreeEx(p,remote,0,MEM_RELEASE);CloseHandle(p);
 wchar_t text[4096]={0};SendMessageW(GetDlgItem(h,110),WM_GETTEXT,4096,(LPARAM)text);return ok&&wcsstr(text,wanted)!=0;
}
int wmain(int argc,wchar_t **argv){
 if(argc==5 && !wcscmp(argv[1],L"--size")){
  wchar_t klass[128],*end=0;swprintf(klass,128,L"MariaStandalone%lsViewer",argv[2]);HWND h=FindWindowW(klass,0);if(!h)return 3;
  long width=wcstol(argv[3],&end,10);if(*end||width<940||width>4096)return 2;
  long height=wcstol(argv[4],&end,10);if(*end||height<440||height>2160)return 2;
  RECT r;GetWindowRect(h,&r);if(!MoveWindow(h,r.left,r.top,(int)width,(int)height,TRUE))return fail("native resize");
  RedrawWindow(h,0,0,RDW_INVALIDATE|RDW_UPDATENOW|RDW_ALLCHILDREN);GetWindowRect(h,&r);
  printf("native bounds=%ld,%ld,%ld,%ld\n",r.left,r.top,r.right-r.left,r.bottom-r.top);return 0;
 }
 if(argc==4 && !wcscmp(argv[1],L"--find")){
  wchar_t klass[128];swprintf(klass,128,L"MariaStandalone%lsViewer",argv[2]);HWND h=FindWindowW(klass,0);if(!h)return 3;
  if(!set_query(h,argv[3])){return fail("search edit content roundtrip");}SendMessageW(GetDlgItem(h,112),BM_CLICK,0,0);return 0;
 }
 if(argc==3 && (!wcscmp(argv[1],L"--probe")||!wcscmp(argv[1],L"--close")||!wcscmp(argv[1],L"--press")||!wcscmp(argv[1],L"--release")||!wcscmp(argv[1],L"--hover")||!wcscmp(argv[1],L"--away")||!wcscmp(argv[1],L"--next")||!wcscmp(argv[1],L"--previous"))){
  wchar_t klass[128];swprintf(klass,128,L"MariaStandalone%lsViewer",argv[2]);HWND h=FindWindowW(klass,0);if(!h)return 3;
  if(!wcscmp(argv[1],L"--close")){PostMessageW(h,WM_CLOSE,0,0);return 0;}
  if(!wcscmp(argv[1],L"--hover")||!wcscmp(argv[1],L"--away")){
   RECT r;GetWindowRect(GetDlgItem(h,101),&r);
   SetCursorPos(!wcscmp(argv[1],L"--hover")?(r.left+r.right)/2:0,!wcscmp(argv[1],L"--hover")?(r.top+r.bottom)/2:0);return 0;
  }
  if(!wcscmp(argv[1],L"--press")||!wcscmp(argv[1],L"--release")){
   int pressed=!wcscmp(argv[1],L"--press");HWND button=GetDlgItem(h,101);
   SendMessageW(button,BM_SETSTATE,pressed,0);RedrawWindow(button,0,0,RDW_INVALIDATE|RDW_UPDATENOW);
   return ((SendMessageW(button,BM_GETSTATE,0,0)&BST_PUSHED)!=0)==pressed?0:5;
  }
  if(!wcscmp(argv[1],L"--next")||!wcscmp(argv[1],L"--previous")){
   SendMessageW(GetDlgItem(h,!wcscmp(argv[1],L"--next")?114:113),BM_CLICK,0,0);return 0;
  }
  RedrawWindow(h,0,0,RDW_INVALIDATE|RDW_UPDATENOW|RDW_ALLCHILDREN);
  wchar_t caption[1024]={0};GetWindowTextW(h,caption,1024);fwprintf(stdout,L"title=%ls\n",caption);
  RECT frame;GetWindowRect(h,&frame);printf("window=%ld,%ld,%ld,%ld theme=%lld\n",frame.left,frame.top,frame.right-frame.left,frame.bottom-frame.top,(long long)SendMessageW(GetDlgItem(h,104),CB_GETCURSEL,0,0));
  RECT button;GetWindowRect(GetDlgItem(h,101),&button);printf("button=%ld,%ld,%ld,%ld\n",button.left,button.top,button.right-button.left,button.bottom-button.top);
  int rows=(int)SendMessageW(GetDlgItem(h,109),LVM_GETITEMCOUNT,0,0);wchar_t status[4096];GetWindowTextW(GetDlgItem(h,108),status,4096);fwprintf(stdout,L"rows=%d status=%ls\n",rows,status);return rows>0?0:4;
 }

 if(argc<6){puts("Usage: native-smoke.exe <desktop.exe> <fixture> <CSV|Parquet> <output-directory> <expected-first-cell>");return 2;}
 wchar_t settings[32768],absolute[32768];
 if(!GetFullPathNameW(argv[4],32768,absolute,0))return fail("absolute isolated settings directory");
 swprintf(settings,32768,L"%ls\\settings-%lu",absolute,GetCurrentProcessId());
 if(!SetEnvironmentVariableW(L"MARIA_VIEWER_SETTINGS_DIR",settings))return fail("isolate appearance from user preferences");
 wchar_t cmd[65536],klass[128];swprintf(cmd,65536,L"\"%ls\" \"%ls\"",argv[1],argv[2]);swprintf(klass,128,L"MariaStandalone%lsViewer",argv[3]);
 HWND previous=FindWindowW(klass,0);if(previous){PostMessageW(previous,WM_CLOSE,0,0);for(int i=0;i<100&&IsWindow(previous);i++)Sleep(20);}
 STARTUPINFOW si={0};si.cb=sizeof(si);PROCESS_INFORMATION pi={0};if(!CreateProcessW(0,cmd,0,0,FALSE,0,0,0,&si,&pi))return fail("launch GUI");
 HWND h=0;for(int i=0;i<300&&!h;i++){h=FindWindowW(klass,0);Sleep(20);}if(!h){TerminateProcess(pi.hProcess,9);return fail("standalone window");}
 HWND status=0,table=0,combo=0;wchar_t text[4096]={0};
 for(int i=0;i<300 && (!status||!table||!combo);i++){
  status=GetDlgItem(h,108);table=GetDlgItem(h,109);combo=GetDlgItem(h,104);Sleep(20);
 }
 if(!status||!table||!combo)return fail("native children created");
 wchar_t button_class[32]={0};HWND open=GetDlgItem(h,101),reload=GetDlgItem(h,102),cancel=GetDlgItem(h,103),copy=GetDlgItem(h,106);
 GetClassNameW(open,button_class,32);GetWindowTextW(open,text,4096);
 if(wcscmp(button_class,L"Button") && wcscmp(button_class,L"BUTTON"))return fail("standard button accessibility identity");
 if((GetWindowLongPtrW(open,GWL_STYLE)&BS_TYPEMASK)==BS_OWNERDRAW)return fail("native button behavior must not be owner-replaced");
 if(!wcsstr(text,L"…"))return fail("Open label requests file input");
 HWND commands[]={open,reload,cancel,copy,GetDlgItem(h,105),GetDlgItem(h,112),GetDlgItem(h,113),GetDlgItem(h,114),GetDlgItem(h,116)};wchar_t used[10]={0};
 for(int i=0;i<9;i++){
  GetWindowTextW(commands[i],text,4096);wchar_t *mark=wcschr(text,L'&');
  if(!mark||!mark[1])return fail("button access key missing");
  wchar_t key=(wchar_t)towlower(mark[1]);if(wcschr(used,key))return fail("duplicate native access key");used[i]=key;
 }
 int rows=0;for(int i=0;i<500;i++){GetWindowTextW(status,text,4096);rows=(int)SendMessageW(table,LVM_GETITEMCOUNT,0,0);if(rows>0)break;Sleep(20);}
 if(rows<=0){fwprintf(stderr,L"status: %ls\n",text);PostMessageW(h,WM_CLOSE,0,0);return fail("real worker populated native table");}
 if(!caption_matches_heading(h,argv[3]))return fail("native title/task switcher identifies the current file");
 printf("PASS: real worker populated owner-data table (%d rows), with native file title\n",rows);
 if(!IsWindowEnabled(reload)||IsWindowEnabled(cancel)||IsWindowEnabled(copy))return fail("ready command availability");
 puts("PASS: standard Button class, ellipsis label, ready/disabled states");
 GUITHREADINFO gui={0};gui.cbSize=sizeof(gui);DWORD thread=GetWindowThreadProcessId(h,0);
 PostMessageW(open,WM_KEYDOWN,VK_TAB,0);
 for(int i=0;i<200;i++){GetGUIThreadInfo(thread,&gui);if(gui.hwndFocus==reload)break;Sleep(10);}
 if(gui.hwndFocus!=reload)return fail("Tab focuses native Reload");
 SendMessageW(reload,WM_KEYDOWN,VK_SPACE,0);
 if(!(SendMessageW(reload,BM_GETSTATE,0,0)&BST_PUSHED))return fail("Space presses native button");
 SendMessageW(reload,WM_KEYUP,VK_SPACE,0);
 for(int i=0;i<500&&!IsWindowEnabled(reload);i++)Sleep(20);
 if(!IsWindowEnabled(reload))return fail("Space reload completes");
 puts("PASS: real Tab focus traversal and native Space activation");
 // Select through the real common control, then read its inspector using the
 // system-marshalled WM_GETTEXT. WM_NOTIFY cannot be sent across processes.
 HANDLE process=OpenProcess(PROCESS_VM_OPERATION|PROCESS_VM_READ|PROCESS_VM_WRITE,FALSE,pi.dwProcessId);
 LVITEMW initial={0};initial.state=LVIS_SELECTED|LVIS_FOCUSED;initial.stateMask=initial.state;
 void *remote=VirtualAllocEx(process,0,sizeof(initial),MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE);
 if(!remote)return fail("allocate native test message");
 WriteProcessMemory(process,remote,&initial,sizeof(initial),0);SendMessageW(table,LVM_SETITEMSTATE,0,(LPARAM)remote);
 VirtualFreeEx(process,remote,0,MEM_RELEASE);CloseHandle(process);
 if(!IsWindowEnabled(copy))return fail("row selection enables native Copy");
 SendMessageW(GetDlgItem(h,110),WM_GETTEXT,4096,(LPARAM)text);
 wchar_t first[1024]={0};if(argc>5){wcscpy(first,argv[5]);if(!wcsstr(text,L"Selected page row 1") || !wcsstr(text,first)){fwprintf(stderr,L"inspector: %ls\n",text);PostMessageW(h,WM_CLOSE,0,0);return fail("first source value preserved in selected row inspector");}}
 fwprintf(stdout,L"PASS: actual selected row inspector contains %ls\n",first);
 const COLORREF bg[]={0xF8FDFF,0x13090B,0x2E1510,0x13090B,0xF8FDFF,0x1C1014};
 for(int i=0;i<6;i++){
  SendMessageW(combo,CB_SETCURSEL,i,0);SendMessageW(h,WM_COMMAND,MAKEWPARAM(104,CBN_SELCHANGE),(LPARAM)combo);
  if((COLORREF)SendMessageW(table,LVM_GETBKCOLOR,0,0)!=bg[i]){PostMessageW(h,WM_CLOSE,0,0);return fail("theme palette not applied to real native table");}
  RedrawWindow(h,0,0,RDW_INVALIDATE|RDW_UPDATENOW|RDW_ALLCHILDREN);
 }
 puts("PASS: six themes applied to the same loaded native table");
 // Selecting a native table row updates the actual inspector, then Copy writes only shown text.
 LVITEMW select={0};select.state=LVIS_SELECTED|LVIS_FOCUSED;select.stateMask=select.state;
 process=OpenProcess(PROCESS_VM_OPERATION|PROCESS_VM_READ|PROCESS_VM_WRITE,FALSE,pi.dwProcessId);remote=VirtualAllocEx(process,0,sizeof(select),MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE);WriteProcessMemory(process,remote,&select,sizeof(select),0);SendMessageW(table,LVM_SETITEMSTATE,0,(LPARAM)remote);VirtualFreeEx(process,remote,0,MEM_RELEASE);CloseHandle(process);
 SendMessageW(copy,BM_CLICK,0,0);if(OpenClipboard(h)){HANDLE data=GetClipboardData(CF_UNICODETEXT);if(!data)return fail("copy shown row");const wchar_t *s=GlobalLock(data);int contains=s&&wcsstr(s,first)!=0;if(s)GlobalUnlock(data);CloseClipboard();if(!contains)return fail("clipboard data preserves shown first value");}else return fail("clipboard open");
 puts("PASS: select row and copy actual shown values");
 if(argc>6 && !wcscmp(argv[6],L"--browse")){
  HWND next=GetDlgItem(h,114),previous=GetDlgItem(h,113),find=GetDlgItem(h,112);
  if(!IsWindowEnabled(next)||IsWindowEnabled(previous))return fail("initial page availability");
  SendMessageW(next,BM_CLICK,0,0);if(!ready(h)||!source_row(h,pi.dwProcessId,L"source row 201)"))return fail("real next page source rows");
  SendMessageW(previous,BM_CLICK,0,0);if(!ready(h)||!source_row(h,pi.dwProcessId,L"source row 1)"))return fail("real previous page");
  if(!set_query(h,L"needle.*")){return fail("search edit content roundtrip");}SendMessageW(find,BM_CLICK,0,0);
  if(!ready(h)||(int)SendMessageW(table,LVM_GETITEMCOUNT,0,0)!=1||!source_row(h,pi.dwProcessId,L"source row 451)"))return fail("global literal search beyond first page");
  if(!set_query(h,L"definitely absent literal.*")){return fail("search edit content roundtrip");}SendMessageW(find,BM_CLICK,0,0);
  if(!ready(h)||(int)SendMessageW(table,LVM_GETITEMCOUNT,0,0)!=0||IsWindowEnabled(next))return fail("search exhaustion, not false matches");
  SendMessageW(GetDlgItem(h,117),WM_GETTEXT,4096,(LPARAM)text);
  if(!IsWindowVisible(GetDlgItem(h,117))||!wcsstr(text,L"Clear search"))return fail("actionable no-match guidance");
  if(!IsWindowEnabled(GetDlgItem(h,116)))return fail("clear search enabled for active filter");
  SendMessageW(GetDlgItem(h,116),BM_CLICK,0,0);
  if(!ready(h)||(int)SendMessageW(table,LVM_GETITEMCOUNT,0,0)!=200||IsWindowEnabled(GetDlgItem(h,116)))return fail("visible Clear search returns first unfiltered page");
  puts("PASS: native next/previous, source row numbers, global literal search beyond row 200 and clear/no-match behavior");
 }
 // Reload and Cancel are recoverable; a further reload must repopulate.
 SendMessageW(reload,BM_CLICK,0,0);if(!IsWindowEnabled(cancel)||IsWindowEnabled(reload))return fail("busy native button states");SendMessageW(cancel,BM_CLICK,0,0);GetWindowTextW(status,text,4096);if(!wcsstr(text,L"Cancelled"))return fail("cancel feedback");
 SendMessageW(h,WM_COMMAND,102,0);for(int i=0;i<500;i++){GetWindowTextW(status,text,4096);if(wcsstr(text,L"page rows"))break;Sleep(20);}if(!wcsstr(text,L"page rows"))return fail("reload after cancel");puts("PASS: cancel/reload recovery");
 // Resize and select a theme, then verify another real process restores both.
 MoveWindow(h,80,60,1040,660,TRUE);RECT remembered;GetWindowRect(h,&remembered);
 SendMessageW(combo,CB_SETCURSEL,3,0);SendMessageW(h,WM_COMMAND,MAKEWPARAM(104,CBN_SELCHANGE),(LPARAM)combo);
 if(!close_gui(h,&pi))return fail("clean close and atomic appearance save");
 swprintf(cmd,65536,L"\"%ls\"",argv[1]);h=launch_gui(cmd,klass,&pi);if(!h)return fail("relaunch without a source file");
 if(!caption_matches_heading(h,argv[3]))return fail("native caption identifies an empty viewer");
 RECT restored;GetWindowRect(h,&restored);
 if(!EqualRect(&remembered,&restored)||SendMessageW(GetDlgItem(h,104),CB_GETCURSEL,0,0)!=3)return fail("size position and theme survive process restart");
 if((int)SendMessageW(GetDlgItem(h,109),LVM_GETITEMCOUNT,0,0)!=0||!IsWindowVisible(GetDlgItem(h,117))||IsWindowEnabled(GetDlgItem(h,112)))return fail("welcome state without restored file history");
 SendMessageW(GetDlgItem(h,117),WM_GETTEXT,4096,(LPARAM)text);if(!wcsstr(text,L"Open a ")||!wcsstr(text,L"read-only"))return fail("functional first-open guidance");
 if(!dialog_check(h,pi.dwProcessId,101,IDCANCEL,0))return fail("visible Open file dialog and harmless cancel");
 if(!dialog_check(h,pi.dwProcessId,105,IDOK,1))return fail("F1 opens real Help and about with Maria credit");
 puts("PASS: remembered normal size, screen position and theme; welcome, Open dialog cancel and F1 help");
 // Resize and maximize in one session: preserve the last normal bounds, not the maximized frame.
 MoveWindow(h,100,80,1060,680,TRUE);GetWindowRect(h,&remembered);ShowWindow(h,SW_MAXIMIZE);
 if(!IsZoomed(h)||!close_gui(h,&pi))return fail("save maximized state");
 swprintf(cmd,65536,L"\"%ls\"",argv[1]);h=launch_gui(cmd,klass,&pi);if(!h||!IsZoomed(h))return fail("maximized state survives restart");
 ShowWindow(h,SW_RESTORE);GetWindowRect(h,&restored);if(!EqualRect(&remembered,&restored))return fail("restore correct normal bounds after maximized restart");
 ShowWindow(h,SW_MINIMIZE);if(!close_gui(h,&pi))return fail("close minimized window");
 swprintf(cmd,65536,L"\"%ls\"",argv[1]);h=launch_gui(cmd,klass,&pi);if(!h||IsIconic(h)||IsZoomed(h))return fail("minimized window reopens normally");
 GetWindowRect(h,&restored);if(!EqualRect(&remembered,&restored))return fail("minimizing does not replace normal bounds");
 if(!close_gui(h,&pi))return fail("clean preferences relaunch exit");
 swprintf(cmd,65536,L"\"%ls\" --theme sumimi",argv[1]);h=launch_gui(cmd,klass,&pi);if(!h||SendMessageW(GetDlgItem(h,104),CB_GETCURSEL,0,0)!=4)return fail("explicit command-line theme overrides saved theme");
 if(!close_gui(h,&pi))return fail("clean standalone GUI exit");
 wchar_t pref_file[32768];swprintf(pref_file,32768,L"%ls\\%ls-viewer\\desktop.json",settings,!wcscmp(argv[3],L"CSV")?L"csv":L"parquet");
 const char *future="{\"version\":999,\"theme\":\"ink\"}";
 FILE *f=_wfopen(pref_file,L"wb");if(!f)return fail("open isolated future-settings fixture");fwrite(future,1,strlen(future),f);fclose(f);
 swprintf(cmd,65536,L"\"%ls\"",argv[1]);h=launch_gui(cmd,klass,&pi);if(!h||SendMessageW(GetDlgItem(h,104),CB_GETCURSEL,0,0)!=0)return fail("unsupported settings use safe default theme");
 if(!close_gui(h,&pi))return fail("close with unsupported settings");
 char saved[256]={0};f=_wfopen(pref_file,L"rb");if(!f)return fail("read isolated appearance file");fread(saved,1,255,f);fclose(f);if(strcmp(saved,future))return fail("unsupported appearance is not silently overwritten");
 swprintf(cmd,65536,L"\"%ls\"",argv[1]);h=launch_gui(cmd,klass,&pi);if(!h)return fail("relaunch for appearance recovery");
 if(!dialog_check(h,pi.dwProcessId,105,IDYES,1))return fail("Help offers explicit appearance reset recovery");
 if(!close_gui(h,&pi))return fail("close after appearance recovery");
 swprintf(cmd,65536,L"\"%ls\"",argv[1]);h=launch_gui(cmd,klass,&pi);if(!h)return fail("relaunch after appearance reset");
 SendMessageW(GetDlgItem(h,108),WM_GETTEXT,4096,(LPARAM)text);if(wcsstr(text,L"could not be read"))return fail("explicit reset repairs appearance for the next launch");
 if(!close_gui(h,&pi))return fail("clean standalone GUI exit");
 puts("PASS: maximize/restore, minimize recovery, theme override, safe unknown-version fallback and explicit repair; clean standalone GUI exit");return 0;
}
