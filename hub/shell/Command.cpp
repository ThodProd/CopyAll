#define UNICODE
#define _UNICODE
#include <windows.h>
#include <shobjidl.h>
#include <shlwapi.h>
#include <string>
#include <new>

// Only collect the selection and start the worker. Never scan files inside Explorer.
static HMODULE module;
static LONG objects=0;
static const CLSID CommandId={0xdb86a616,0xf972,0x4d28,{0x99,0xb6,0xcb,0x10,0x87,0x62,0x1b,0x51}};
static std::wstring WorkerPath(){wchar_t path[32768];DWORD n=GetModuleFileNameW(module,path,32768);if(!n||n>=32768)return {};std::wstring p(path,n);return p.substr(0,p.find_last_of(L'\\')+1)+L"CopyAll.Worker.exe";}
static std::wstring Quote(const wchar_t* path){std::wstring p(path);size_t trailing=0;for(size_t i=p.size();i>0&&p[i-1]==L'\\';--i)++trailing;return L"\""+p+std::wstring(trailing,L'\\')+L"\"";}
class Command final:public IExplorerCommand {
 LONG refs=1;
 public:
 Command(){InterlockedIncrement(&objects);}~Command(){InterlockedDecrement(&objects);}
 HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid,void** p) override{if(!p)return E_POINTER;*p=nullptr;if(iid==IID_IUnknown||iid==IID_IExplorerCommand){*p=static_cast<IExplorerCommand*>(this);AddRef();return S_OK;}return E_NOINTERFACE;}
 ULONG STDMETHODCALLTYPE AddRef() override{return InterlockedIncrement(&refs);}
 ULONG STDMETHODCALLTYPE Release() override{ULONG n=InterlockedDecrement(&refs);if(!n)delete this;return n;}
 HRESULT STDMETHODCALLTYPE GetTitle(IShellItemArray*,PWSTR* text) override{return SHStrDupW(L"CopyAll — копировать все пути",text);}
 HRESULT STDMETHODCALLTYPE GetIcon(IShellItemArray*,PWSTR* text) override{auto p=WorkerPath()+L",0";return SHStrDupW(p.c_str(),text);}
 HRESULT STDMETHODCALLTYPE GetToolTip(IShellItemArray*,PWSTR* text) override{return SHStrDupW(L"Скопировать пути папки и всех вложенных файлов",text);}
 HRESULT STDMETHODCALLTYPE GetCanonicalName(GUID* id) override{if(!id)return E_POINTER;*id=CommandId;return S_OK;}
 HRESULT STDMETHODCALLTYPE GetState(IShellItemArray* items,BOOL,EXPCMDSTATE* state) override{if(!state)return E_POINTER;*state=ECS_HIDDEN;if(items){SFGAOF flags=0;if(SUCCEEDED(items->GetAttributes(SIATTRIBFLAGS_AND,SFGAO_FILESYSTEM,&flags))&&(flags&SFGAO_FILESYSTEM))*state=ECS_ENABLED;}return S_OK;}
 HRESULT STDMETHODCALLTYPE Invoke(IShellItemArray* items,IBindCtx*) override{
  if(!items)return E_INVALIDARG;DWORD count=0;HRESULT hr=items->GetCount(&count);if(FAILED(hr))return hr;if(!count)return E_INVALIDARG;
  auto exe=WorkerPath();if(exe.empty())return E_FAIL;auto line=Quote(exe.c_str())+L" --context-copy";
  for(DWORD i=0;i<count;++i){IShellItem* item=nullptr;hr=items->GetItemAt(i,&item);if(FAILED(hr))return hr;PWSTR path=nullptr;hr=item->GetDisplayName(SIGDN_FILESYSPATH,&path);item->Release();if(FAILED(hr))return hr;line+=L" "+Quote(path);CoTaskMemFree(path);if(line.size()>32760)return HRESULT_FROM_WIN32(ERROR_BUFFER_OVERFLOW);}
  STARTUPINFOW si={sizeof(si)};PROCESS_INFORMATION pi={};if(!CreateProcessW(exe.c_str(),line.data(),nullptr,nullptr,FALSE,CREATE_NO_WINDOW,nullptr,nullptr,&si,&pi))return HRESULT_FROM_WIN32(GetLastError());CloseHandle(pi.hThread);CloseHandle(pi.hProcess);return S_OK;
 }
 HRESULT STDMETHODCALLTYPE GetFlags(EXPCMDFLAGS* flags) override{if(!flags)return E_POINTER;*flags=ECF_DEFAULT;return S_OK;}
 HRESULT STDMETHODCALLTYPE EnumSubCommands(IEnumExplorerCommand** commands) override{if(commands)*commands=nullptr;return E_NOTIMPL;}
};
class Factory final:public IClassFactory{
 LONG refs=1;
 public:
 Factory(){InterlockedIncrement(&objects);}~Factory(){InterlockedDecrement(&objects);}
 HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid,void** p) override{if(!p)return E_POINTER;*p=nullptr;if(iid==IID_IUnknown||iid==IID_IClassFactory){*p=static_cast<IClassFactory*>(this);AddRef();return S_OK;}return E_NOINTERFACE;}
 ULONG STDMETHODCALLTYPE AddRef() override{return InterlockedIncrement(&refs);}ULONG STDMETHODCALLTYPE Release() override{ULONG n=InterlockedDecrement(&refs);if(!n)delete this;return n;}
 HRESULT STDMETHODCALLTYPE CreateInstance(IUnknown* outer,REFIID iid,void** p) override{if(outer)return CLASS_E_NOAGGREGATION;auto object=new(std::nothrow)Command();if(!object)return E_OUTOFMEMORY;auto hr=object->QueryInterface(iid,p);object->Release();return hr;}
 HRESULT STDMETHODCALLTYPE LockServer(BOOL lock) override{if(lock)InterlockedIncrement(&objects);else InterlockedDecrement(&objects);return S_OK;}
};
extern "C" HRESULT WINAPI DllGetClassObject(REFCLSID id,REFIID iid,void** p){if(id!=CommandId)return CLASS_E_CLASSNOTAVAILABLE;auto f=new(std::nothrow)Factory();if(!f)return E_OUTOFMEMORY;auto hr=f->QueryInterface(iid,p);f->Release();return hr;}
extern "C" HRESULT WINAPI DllCanUnloadNow(){return objects==0?S_OK:S_FALSE;}
BOOL WINAPI DllMain(HINSTANCE instance,DWORD reason,LPVOID){if(reason==DLL_PROCESS_ATTACH){module=instance;DisableThreadLibraryCalls(instance);}return TRUE;}
