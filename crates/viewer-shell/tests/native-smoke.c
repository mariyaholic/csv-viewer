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
 if(argc==4 && !wcscmp(argv[1],L"--find")){
  wchar_t klass[128];swprintf(klass,128,L"MariaStandalone%lsViewer",argv[2]);HWND h=FindWindowW(klass,0);if(!h)return 3;
  if(!set_query(h,argv[3]))return fail("search edit content roundtrip");SendMessageW(GetDlgItem(h,112),BM_CLICK,0,0);return 0;
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
  RECT button;GetWindowRect(GetDlgItem(h,101),&button);printf("button=%ld,%ld,%ld,%ld\n",button.left,button.top,button.right-button.left,button.bottom-button.top);
  int rows=(int)SendMessageW(GetDlgItem(h,109),LVM_GETITEMCOUNT,0,0);wchar_t status[4096];GetWindowTextW(GetDlgItem(h,108),status,4096);fwprintf(stdout,L"rows=%d status=%ls\n",rows,status);return rows>0?0:4;
 }

 if(argc<6){puts("Usage: native-smoke.exe <desktop.exe> <fixture> <CSV|Parquet> <output-directory> <expected-first-cell>");return 2;}
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
 HWND commands[]={open,reload,cancel,copy,GetDlgItem(h,105),GetDlgItem(h,112),GetDlgItem(h,113),GetDlgItem(h,114)};wchar_t used[9]={0};
 for(int i=0;i<8;i++){
  GetWindowTextW(commands[i],text,4096);wchar_t *mark=wcschr(text,L'&');
  if(!mark||!mark[1])return fail("button access key missing");
  wchar_t key=(wchar_t)towlower(mark[1]);if(wcschr(used,key))return fail("duplicate native access key");used[i]=key;
 }
 int rows=0;for(int i=0;i<500;i++){GetWindowTextW(status,text,4096);rows=(int)SendMessageW(table,LVM_GETITEMCOUNT,0,0);if(rows>0)break;Sleep(20);}
 if(rows<=0){fwprintf(stderr,L"status: %ls\n",text);PostMessageW(h,WM_CLOSE,0,0);return fail("real worker populated native table");}
 printf("PASS: real worker populated owner-data table (%d rows)\n",rows);
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
  if(!set_query(h,L"needle.*"))return fail("search edit content roundtrip");SendMessageW(find,BM_CLICK,0,0);
  if(!ready(h)||(int)SendMessageW(table,LVM_GETITEMCOUNT,0,0)!=1||!source_row(h,pi.dwProcessId,L"source row 451)"))return fail("global literal search beyond first page");
  if(!set_query(h,L"definitely absent literal.*"))return fail("search edit content roundtrip");SendMessageW(find,BM_CLICK,0,0);
  if(!ready(h)||(int)SendMessageW(table,LVM_GETITEMCOUNT,0,0)!=0||IsWindowEnabled(next))return fail("search exhaustion, not false matches");
  if(!set_query(h,L""))return fail("search edit content roundtrip");SendMessageW(find,BM_CLICK,0,0);if(!ready(h)||(int)SendMessageW(table,LVM_GETITEMCOUNT,0,0)!=200)return fail("clear search returns first page");
  puts("PASS: native next/previous, source row numbers, global literal search beyond row 200 and clear/no-match behavior");
 }
 // Reload and Cancel are recoverable; a further reload must repopulate.
 SendMessageW(reload,BM_CLICK,0,0);if(!IsWindowEnabled(cancel)||IsWindowEnabled(reload))return fail("busy native button states");SendMessageW(cancel,BM_CLICK,0,0);GetWindowTextW(status,text,4096);if(!wcsstr(text,L"Cancelled"))return fail("cancel feedback");
 SendMessageW(h,WM_COMMAND,102,0);for(int i=0;i<500;i++){GetWindowTextW(status,text,4096);if(wcsstr(text,L"page rows"))break;Sleep(20);}if(!wcsstr(text,L"page rows"))return fail("reload after cancel");puts("PASS: cancel/reload recovery");
 PostMessageW(h,WM_CLOSE,0,0);if(WaitForSingleObject(pi.hProcess,5000)!=WAIT_OBJECT_0){TerminateProcess(pi.hProcess,9);return fail("clean close");}DWORD code;GetExitCodeProcess(pi.hProcess,&code);CloseHandle(pi.hThread);CloseHandle(pi.hProcess);if(code)return fail("GUI exit status");puts("PASS: clean standalone GUI exit");return 0;
}
