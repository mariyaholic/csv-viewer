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
static int fail(const char *what){fprintf(stderr,"FAIL: %s (Win32 %lu)\n",what,GetLastError());return 1;}
int wmain(int argc,wchar_t **argv){
 if(argc==3 && (!wcscmp(argv[1],L"--probe")||!wcscmp(argv[1],L"--close"))){
  wchar_t klass[128];swprintf(klass,128,L"MariaStandalone%lsViewer",argv[2]);HWND h=FindWindowW(klass,0);if(!h)return 3;
  if(!wcscmp(argv[1],L"--close")){PostMessageW(h,WM_CLOSE,0,0);return 0;}
  int rows=(int)SendMessageW(GetDlgItem(h,109),LVM_GETITEMCOUNT,0,0);wchar_t status[4096];GetWindowTextW(GetDlgItem(h,108),status,4096);fwprintf(stdout,L"rows=%d status=%ls\n",rows,status);return rows>0?0:4;
 }

 if(argc<6){puts("Usage: native-smoke.exe <desktop.exe> <fixture> <CSV|Parquet> <output-directory> <expected-first-cell>");return 2;}
 wchar_t cmd[65536],klass[128];swprintf(cmd,65536,L"\"%ls\" \"%ls\"",argv[1],argv[2]);swprintf(klass,128,L"MariaStandalone%lsViewer",argv[3]);
 HWND previous=FindWindowW(klass,0);if(previous){PostMessageW(previous,WM_CLOSE,0,0);for(int i=0;i<100&&IsWindow(previous);i++)Sleep(20);}
 STARTUPINFOW si={0};si.cb=sizeof(si);PROCESS_INFORMATION pi={0};if(!CreateProcessW(0,cmd,0,0,FALSE,0,0,0,&si,&pi))return fail("launch GUI");
 HWND h=0;for(int i=0;i<300&&!h;i++){h=FindWindowW(klass,0);Sleep(20);}if(!h){TerminateProcess(pi.hProcess,9);return fail("standalone window");}
 HWND status=GetDlgItem(h,108),table=GetDlgItem(h,109),combo=GetDlgItem(h,104);wchar_t text[4096]={0};
 int rows=0;for(int i=0;i<500;i++){GetWindowTextW(status,text,4096);rows=(int)SendMessageW(table,LVM_GETITEMCOUNT,0,0);if(rows>0)break;Sleep(20);}
 if(rows<=0){fwprintf(stderr,L"status: %ls\n",text);PostMessageW(h,WM_CLOSE,0,0);return fail("real worker populated native table");}
 printf("PASS: real worker populated owner-data table (%d rows)\n",rows);
 // Select through the real common control, then read its inspector using the
 // system-marshalled WM_GETTEXT. WM_NOTIFY cannot be sent across processes.
 HANDLE process=OpenProcess(PROCESS_VM_OPERATION|PROCESS_VM_READ|PROCESS_VM_WRITE,FALSE,pi.dwProcessId);
 LVITEMW initial={0};initial.state=LVIS_SELECTED|LVIS_FOCUSED;initial.stateMask=initial.state;
 void *remote=VirtualAllocEx(process,0,sizeof(initial),MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE);
 if(!remote)return fail("allocate native test message");
 WriteProcessMemory(process,remote,&initial,sizeof(initial),0);SendMessageW(table,LVM_SETITEMSTATE,0,(LPARAM)remote);
 VirtualFreeEx(process,remote,0,MEM_RELEASE);CloseHandle(process);
 SendMessageW(GetDlgItem(h,110),WM_GETTEXT,4096,(LPARAM)text);
 wchar_t first[1024]={0};if(argc>5){wcscpy(first,argv[5]);if(!wcsstr(text,L"Selected sample row 1") || !wcsstr(text,first)){fwprintf(stderr,L"inspector: %ls\n",text);PostMessageW(h,WM_CLOSE,0,0);return fail("first source value preserved in selected row inspector");}}
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
 SendMessageW(h,WM_COMMAND,106,0);if(OpenClipboard(h)){HANDLE data=GetClipboardData(CF_UNICODETEXT);if(!data)return fail("copy shown row");const wchar_t *s=GlobalLock(data);int contains=s&&wcsstr(s,first)!=0;if(s)GlobalUnlock(data);CloseClipboard();if(!contains)return fail("clipboard data preserves shown first value");}else return fail("clipboard open");
 puts("PASS: select row and copy actual shown values");
 // Reload and Cancel are recoverable; a further reload must repopulate.
 SendMessageW(h,WM_COMMAND,102,0);SendMessageW(h,WM_COMMAND,103,0);GetWindowTextW(status,text,4096);if(!wcsstr(text,L"Cancelled"))return fail("cancel feedback");
 SendMessageW(h,WM_COMMAND,102,0);for(int i=0;i<500;i++){GetWindowTextW(status,text,4096);if(wcsstr(text,L"sample"))break;Sleep(20);}if(!wcsstr(text,L"sample"))return fail("reload after cancel");puts("PASS: cancel/reload recovery");
 PostMessageW(h,WM_CLOSE,0,0);if(WaitForSingleObject(pi.hProcess,5000)!=WAIT_OBJECT_0){TerminateProcess(pi.hProcess,9);return fail("clean close");}DWORD code;GetExitCodeProcess(pi.hProcess,&code);CloseHandle(pi.hThread);CloseHandle(pi.hProcess);if(code)return fail("GUI exit status");puts("PASS: clean standalone GUI exit");return 0;
}
