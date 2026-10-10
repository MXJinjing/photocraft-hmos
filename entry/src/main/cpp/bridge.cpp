#include <ace/xcomponent/native_interface_xcomponent.h>
#include <native_window/external_window.h>
#include <native_vsync/native_vsync.h>
#include <napi/native_api.h>
#include <hilog/log.h>
#include <arkui/ui_input_event.h>
#include <atomic>
#include <algorithm>
#include <cmath>
#include <cstring>
#include <mutex>
#include <string>
#include <vector>

extern "C" {
using Notify = void (*)(uint64_t,const uint8_t*,size_t,const uint8_t*,size_t);
bool pc_init(const uint8_t*,size_t,Notify);
void pc_attach(uintptr_t,uint32_t,uint32_t);
void pc_resize(uint32_t,uint32_t);
void pc_detach();
void pc_frame();
void pc_active(bool);
void pc_close();
void pc_input(const uint8_t*,size_t);
void pc_reply(uint64_t,bool,const uint8_t*,size_t,const uint8_t*,size_t);
void pc_open(const uint8_t*,size_t,const uint8_t*,size_t);
void pc_command(const uint8_t*,size_t);
}
namespace {
std::mutex notifyMutex;
napi_threadsafe_function notifyFn = nullptr;
OH_NativeVSync* vsync = nullptr;
std::atomic<bool> framePending(false);
std::atomic<bool> attached(false);
std::atomic<int32_t> touchId(-1);
std::atomic<bool> penContact(false);
std::atomic<bool> fingerContact(false);
struct Request {uint64_t id;std::string json;std::vector<uint8_t> data;};
// 在 ArkTS 线程消费并释放请求；Rust 回调中的借用数据已由 NotifyRust 复制。
void CallJS(napi_env env,napi_value callback,void*,void* ptr) {
    auto* r=static_cast<Request*>(ptr);
    if (env && callback) {
        napi_value args[3],result,receiver;
        napi_create_double(env,static_cast<double>(r->id),&args[0]);
        napi_create_string_utf8(env,r->json.data(),r->json.size(),&args[1]);
        void* data=nullptr;
        napi_create_arraybuffer(env,r->data.size(),&data,&args[2]);
        if (!r->data.empty()) std::memcpy(data,r->data.data(),r->data.size());
        napi_get_undefined(env,&receiver);
        if (napi_call_function(env,receiver,callback,3,args,&result)!=napi_ok && r->id) {
            const std::string error="platform callback failed";
            pc_reply(r->id,false,reinterpret_cast<const uint8_t*>(error.data()),error.size(),nullptr,0);
        }
    } else if(r->id) {
        const std::string error="cancelled";
        pc_reply(r->id,false,reinterpret_cast<const uint8_t*>(error.data()),error.size(),nullptr,0);
    }
    delete r;
}
void NotifyRust(uint64_t id,const uint8_t* json,size_t n,const uint8_t* data,size_t size) {
    // Rust worker 不直接调用 JS，通过线程安全队列将平台请求交给 ArkTS 主线程。
    auto* r=new Request{id,std::string(reinterpret_cast<const char*>(json),n),{}};
    // IME snapshots contain user text and can arrive on every cursor movement.
    if(r->json.find("\"kind\":\"ime\"")==std::string::npos)
        OH_LOG_Print(LOG_APP, LOG_INFO, 0xD001, "PhotoCraft", "Rust notification %{public}s", r->json.c_str());
    if(size) r->data.assign(data,data+size);
    std::lock_guard<std::mutex> lock(notifyMutex);
    if(!notifyFn || napi_call_threadsafe_function(notifyFn,r,napi_tsfn_nonblocking)!=napi_ok) {
        delete r;
        if(id) {const std::string error="cancelled";pc_reply(id,false,reinterpret_cast<const uint8_t*>(error.data()),error.size(),nullptr,0);}
    }
}
// VSync 只发帧消息，实际 egui 更新和 GPU 绘制仍由 Rust worker 串行执行。
void Vsync(long long,void*) {framePending=false;if(attached)pc_frame();}
std::string String(napi_env env,napi_value v) {
    size_t len=0;
    if(!v || napi_get_value_string_utf8(env,v,nullptr,0,&len)!=napi_ok || len>1024*1024)return {};
    std::vector<char> data(len+1);
    if(napi_get_value_string_utf8(env,v,data.data(),data.size(),&len)!=napi_ok)return {};
    return std::string(data.data(),len);
}
napi_value Undefined(napi_env env){napi_value v;napi_get_undefined(env,&v);return v;}
void Json(const std::string& s){pc_input(reinterpret_cast<const uint8_t*>(s.data()),s.size());}
void SurfaceCreated(OH_NativeXComponent* c,void* window) {
    OH_LOG_Print(LOG_APP, LOG_INFO, 0xD001, "PhotoCraft", "Surface created");
    uint64_t w=0,h=0;
    if(!window||OH_NativeXComponent_GetXComponentSize(c,window,&w,&h)!=OH_NATIVEXCOMPONENT_RESULT_SUCCESS)return;
    auto ref=OH_NativeWindow_NativeObjectReference(window);
    OH_LOG_Print(LOG_APP, LOG_INFO, 0xD001, "PhotoCraft", "Window %{public}llu x %{public}llu reference %{public}d", static_cast<unsigned long long>(w),static_cast<unsigned long long>(h),ref);
    if(ref!=0)return;
    // 将取得的原生窗口引用转交 Rust 的 Window；后者负责在 surface 释放后解除引用。
    attached=true;
    pc_attach(reinterpret_cast<uintptr_t>(window),static_cast<uint32_t>(w),static_cast<uint32_t>(h));
}
void SurfaceChanged(OH_NativeXComponent* c,void* window) {
    uint64_t w=0,h=0;
    if(OH_NativeXComponent_GetXComponentSize(c,window,&w,&h)==OH_NATIVEXCOMPONENT_RESULT_SUCCESS)pc_resize(static_cast<uint32_t>(w),static_cast<uint32_t>(h));
}
void SurfaceDestroyed(OH_NativeXComponent*,void*) {attached=false;touchId=-1;penContact=false;fingerContact=false;pc_detach();}
void Touch(OH_NativeXComponent* c,void* window) {
    OH_NativeXComponent_TouchEvent e{};
    if(OH_NativeXComponent_GetTouchEvent(c,window,&e)!=OH_NATIVEXCOMPONENT_RESULT_SUCCESS)return;
    uint32_t index=0;
    while(index<e.numPoints && index<OH_NATIVE_XCOMPONENT_MAX_TOUCH_POINTS_NUMBER && e.touchPoints[index].id!=e.id)++index;
    OH_NativeXComponent_TouchPointToolType tool=OH_NATIVEXCOMPONENT_TOOL_TYPE_FINGER;
    if(index<e.numPoints && index<OH_NATIVE_XCOMPONENT_MAX_TOUCH_POINTS_NUMBER)
        OH_NativeXComponent_GetTouchPointToolType(c,index,&tool);
    bool pen=tool==OH_NATIVEXCOMPONENT_TOOL_TYPE_PEN || tool==OH_NATIVEXCOMPONENT_TOOL_TYPE_RUBBER ||
             tool==OH_NATIVEXCOMPONENT_TOOL_TYPE_BRUSH || tool==OH_NATIVEXCOMPONENT_TOOL_TYPE_PENCIL ||
             tool==OH_NATIVEXCOMPONENT_TOOL_TYPE_AIRBRUSH;
    if(!pen) {
        if(e.type==OH_NATIVEXCOMPONENT_CANCEL) {
            fingerContact=false;
            if(!penContact)Json("{\"kind\":\"blur\"}");
            return;
        }
        std::string points;
        for(uint32_t i=0;i<e.numPoints && i<OH_NATIVE_XCOMPONENT_MAX_TOUCH_POINTS_NUMBER;++i) {
            const auto& point=e.touchPoints[i];
            OH_NativeXComponent_TouchPointToolType pointTool=OH_NATIVEXCOMPONENT_TOOL_TYPE_FINGER;
            OH_NativeXComponent_GetTouchPointToolType(c,i,&pointTool);
            // isPressed is false on MOVE in the simulator; use event phases for contact lifetime.
            if(pointTool!=OH_NATIVEXCOMPONENT_TOOL_TYPE_FINGER ||
               point.type==OH_NATIVEXCOMPONENT_UP || point.type==OH_NATIVEXCOMPONENT_CANCEL ||
               (point.id==e.id && e.type==OH_NATIVEXCOMPONENT_UP) ||
               !std::isfinite(point.x) || !std::isfinite(point.y))continue;
            if(!points.empty())points+=",";
            points+="{\"id\":"+std::to_string(static_cast<uint32_t>(point.id))+",\"x\":"+std::to_string(point.x)+",\"y\":"+std::to_string(point.y)+"}";
        }
        fingerContact=!points.empty();
        Json("{\"kind\":\"touch\",\"points\":["+points+"]}");
        return;
    }
    int action=2;
    if(e.type==OH_NATIVEXCOMPONENT_DOWN){
        if(touchId!=-1) {
            if(!pen || penContact)return;
            // A palm must never prevent a pen from taking ownership of the canvas.
            Json("{\"kind\":\"pointer\",\"action\":3}");
        }
        touchId=e.id;penContact=pen;action=0;
    }
    else if(e.type==OH_NATIVEXCOMPONENT_UP)action=1;
    else if(e.type==OH_NATIVEXCOMPONENT_CANCEL)action=3;
    if(touchId!=e.id)return;
    float tiltX=0,tiltY=0;
    if(pen && index<e.numPoints && index<OH_NATIVE_XCOMPONENT_MAX_TOUCH_POINTS_NUMBER) {
        OH_NativeXComponent_GetTouchPointTiltX(c,index,&tiltX);
        OH_NativeXComponent_GetTouchPointTiltY(c,index,&tiltY);
    }
    auto finite=[](float value,float fallback){return std::isfinite(value)?value:fallback;};
    std::string sample=pen ? "{\"pressure\":"+std::to_string(std::clamp(finite(e.force,1),0.0f,1.0f))+
        ",\"tiltX\":"+std::to_string(finite(tiltX,0))+",\"tiltY\":"+std::to_string(finite(tiltY,0))+
        ",\"eraser\":"+(tool==OH_NATIVEXCOMPONENT_TOOL_TYPE_RUBBER?"true":"false")+"}" : "null";
    Json("{\"kind\":\"pointer\",\"source\":\""+std::string(pen?"pen":"touch")+"\",\"sample\":"+sample+
         ",\"action\":"+std::to_string(action)+",\"x\":"+std::to_string(e.x)+",\"y\":"+std::to_string(e.y)+"}");
    if(action==1||action==3){touchId=-1;penContact=false;}
}
void Mouse(OH_NativeXComponent* c,void* window) {
    if(penContact || fingerContact)return;
    OH_NativeXComponent_MouseEvent e{};
    if(OH_NativeXComponent_GetMouseEvent(c,window,&e)!=OH_NATIVEXCOMPONENT_RESULT_SUCCESS)return;
    int action=e.action==OH_NATIVEXCOMPONENT_MOUSE_PRESS?0:e.action==OH_NATIVEXCOMPONENT_MOUSE_RELEASE?1:2;
    int button=e.button==OH_NATIVEXCOMPONENT_RIGHT_BUTTON?1:e.button==OH_NATIVEXCOMPONENT_MIDDLE_BUTTON?2:0;
    Json("{\"kind\":\"pointer\",\"action\":"+std::to_string(action)+",\"button\":"+std::to_string(button)+",\"x\":"+std::to_string(e.x)+",\"y\":"+std::to_string(e.y)+"}");
}
bool Key(OH_NativeXComponent* c,void*) {
    OH_NativeXComponent_KeyEvent* event=nullptr;
    OH_NativeXComponent_KeyAction action=OH_NATIVEXCOMPONENT_KEY_ACTION_UNKNOWN;
    OH_NativeXComponent_KeyCode code=static_cast<OH_NativeXComponent_KeyCode>(-1);
    if(OH_NativeXComponent_GetKeyEvent(c,&event)!=0 || !event ||
       OH_NativeXComponent_GetKeyEventAction(event,&action)!=0 ||
       OH_NativeXComponent_GetKeyEventCode(event,&code)!=0)return false;
    uint64_t modifiers=0;bool caps=false;
    OH_NativeXComponent_GetKeyEventModifierKeyStates(event,&modifiers);
    OH_NativeXComponent_GetKeyEventCapsLockState(event,&caps);
    bool ctrl=modifiers&1,shift=modifiers&2,alt=modifiers&4;
    int key=static_cast<int>(code);
    std::string text;
    if(!ctrl&&!alt&&action==OH_NATIVEXCOMPONENT_KEY_ACTION_DOWN) {
        if(key>=2017&&key<=2042)text.push_back(static_cast<char>((shift!=caps?'A':'a')+key-2017));
        else if(key>=2000&&key<=2009)text.push_back(shift?")!@#$%^&*("[key-2000]:static_cast<char>('0'+key-2000));
        else if(key==2050)text=" ";
        else {
            const int codes[]={2043,2044,2056,2057,2058,2059,2060,2061,2062,2063,2064};
            const char* plain=",.`-=[]\\;'/";
            const char* shifted="<>~_+{}|:\"?";
            for(size_t i=0;i<sizeof(codes)/sizeof(codes[0]);++i)if(key==codes[i])text.push_back(shift?shifted[i]:plain[i]);
        }
    }
    if(text=="\\")text="\\\\";
    else if(text=="\"")text="\\\"";
    Json("{\"kind\":\"key\",\"action\":"+std::to_string(static_cast<int>(action))+
         ",\"code\":"+std::to_string(key)+",\"ctrl\":"+(ctrl?"true":"false")+
         ",\"shift\":"+(shift?"true":"false")+",\"alt\":"+(alt?"true":"false")+
         ",\"text\":\""+text+"\"}");
    return true;
}
void Axis(OH_NativeXComponent*,ArkUI_UIInputEvent* event,ArkUI_UIInputEvent_Type type) {
    if(!event||type!=ARKUI_UIINPUTEVENT_TYPE_AXIS || penContact)return;
    double x=-OH_ArkUI_AxisEvent_GetHorizontalAxisValue(event);
    double y=-OH_ArkUI_AxisEvent_GetVerticalAxisValue(event);
    double zoom=OH_ArkUI_AxisEvent_GetPinchAxisScaleValue(event);
    float px=OH_ArkUI_PointerEvent_GetX(event),py=OH_ArkUI_PointerEvent_GetY(event);
    if(!std::isfinite(x)||!std::isfinite(y)||!std::isfinite(px)||!std::isfinite(py))return;
    if(!std::isfinite(zoom)||zoom<0)zoom=0;
    Json("{\"kind\":\"axis\",\"action\":"+std::to_string(OH_ArkUI_AxisEvent_GetAxisAction(event))+
         ",\"x\":"+std::to_string(px)+",\"y\":"+std::to_string(py)+",\"dx\":"+std::to_string(x)+
         ",\"dy\":"+std::to_string(y)+",\"zoom\":"+std::to_string(zoom)+"}");
}
void Blur(OH_NativeXComponent*,void*) {touchId=-1;penContact=false;fingerContact=false;Json("{\"kind\":\"blur\"}");}
void Hover(OH_NativeXComponent*,bool isHover) {if(!isHover && !penContact && !fingerContact)Json("{\"kind\":\"blur\"}");}
OH_NativeXComponent_Callback callbacks{SurfaceCreated,SurfaceChanged,SurfaceDestroyed,Touch};
OH_NativeXComponent_MouseEvent_Callback mouseCallbacks{Mouse,Hover};

napi_value InitHost(napi_env env,napi_callback_info info) {
    OH_LOG_Print(LOG_APP, LOG_INFO, 0xD001, "PhotoCraft", "Initializing Rust host");
    size_t n=2;napi_value args[2];napi_get_cb_info(env,info,&n,args,nullptr,nullptr);
    if(n!=2)return Undefined(env);
    auto config=String(env,args[0]);
    OH_LOG_Print(LOG_APP, LOG_INFO, 0xD001, "PhotoCraft", "Config %{public}s", config.c_str());
    {
        std::lock_guard<std::mutex> lock(notifyMutex);
        if(notifyFn)napi_release_threadsafe_function(notifyFn,napi_tsfn_abort);
        napi_value name;napi_create_string_utf8(env,"PhotoCraft platform",NAPI_AUTO_LENGTH,&name);
        auto status=napi_create_threadsafe_function(env,args[1],nullptr,name,0,1,nullptr,nullptr,nullptr,CallJS,&notifyFn);
        OH_LOG_Print(LOG_APP, LOG_INFO, 0xD001, "PhotoCraft", "TSFN status %{public}d", static_cast<int>(status));
        if(status!=napi_ok)return Undefined(env);
        napi_unref_threadsafe_function(env,notifyFn);
    }
    if(!vsync)vsync=OH_NativeVSync_Create("PhotoCraft",10);
    bool started=pc_init(reinterpret_cast<const uint8_t*>(config.data()),config.size(),NotifyRust);
    OH_LOG_Print(LOG_APP, LOG_INFO, 0xD001, "PhotoCraft", "Rust started %{public}d", started);
    if(!started) {
        const std::string error="Native configuration failed";
        const std::string json="{\"kind\":\"error\",\"text\":\"Native configuration failed\"}";
        NotifyRust(0,reinterpret_cast<const uint8_t*>(json.data()),json.size(),nullptr,0);
    }
    return Undefined(env);
}
napi_value Input(napi_env env,napi_callback_info info) {
    size_t n=1;napi_value arg;napi_get_cb_info(env,info,&n,&arg,nullptr,nullptr);
    if(n){auto s=String(env,arg);Json(s);}return Undefined(env);
}
napi_value Command(napi_env env,napi_callback_info info) {
    size_t n=1;napi_value arg;napi_get_cb_info(env,info,&n,&arg,nullptr,nullptr);
    if(n){auto s=String(env,arg);pc_command(reinterpret_cast<const uint8_t*>(s.data()),s.size());}return Undefined(env);
}
napi_value Reply(napi_env env,napi_callback_info info) {
    size_t n=4;napi_value args[4];napi_get_cb_info(env,info,&n,args,nullptr,nullptr);
    if(n!=4)return Undefined(env);
    double id=0;bool ok=false;void* data=nullptr;size_t size=0;
    napi_get_value_double(env,args[0],&id);napi_get_value_bool(env,args[1],&ok);
    auto text=String(env,args[2]);napi_get_arraybuffer_info(env,args[3],&data,&size);
    pc_reply(static_cast<uint64_t>(id),ok,reinterpret_cast<const uint8_t*>(text.data()),text.size(),static_cast<uint8_t*>(data),size);
    return Undefined(env);
}
napi_value Open(napi_env env,napi_callback_info info) {
    size_t n=2;napi_value args[2];napi_get_cb_info(env,info,&n,args,nullptr,nullptr);
    if(n!=2)return Undefined(env);
    auto name=String(env,args[0]);void* data=nullptr;size_t size=0;napi_get_arraybuffer_info(env,args[1],&data,&size);
    pc_open(reinterpret_cast<const uint8_t*>(name.data()),name.size(),static_cast<uint8_t*>(data),size);return Undefined(env);
}
napi_value Active(napi_env env,napi_callback_info info) {
    size_t n=1;napi_value arg;bool active=false;napi_get_cb_info(env,info,&n,&arg,nullptr,nullptr);
    if(n)napi_get_value_bool(env,arg,&active);pc_active(active);return Undefined(env);
}
napi_value Close(napi_env env,napi_callback_info){pc_close();return Undefined(env);}
void Cleanup(void*) {
    attached=false;pc_active(false);pc_detach();
    std::lock_guard<std::mutex> lock(notifyMutex);
    if(notifyFn){napi_release_threadsafe_function(notifyFn,napi_tsfn_abort);notifyFn=nullptr;}
}
napi_value Init(napi_env env,napi_value exports) {
    OH_LOG_Print(LOG_APP, LOG_INFO, 0xD001, "PhotoCraft", "Loading NAPI module");
    const napi_property_descriptor props[]={
        {"init",nullptr,InitHost,nullptr,nullptr,nullptr,napi_default,nullptr},
        {"input",nullptr,Input,nullptr,nullptr,nullptr,napi_default,nullptr},
        {"reply",nullptr,Reply,nullptr,nullptr,nullptr,napi_default,nullptr},
        {"open",nullptr,Open,nullptr,nullptr,nullptr,napi_default,nullptr},
        {"active",nullptr,Active,nullptr,nullptr,nullptr,napi_default,nullptr},
        {"close",nullptr,Close,nullptr,nullptr,nullptr,napi_default,nullptr},
        {"command",nullptr,Command,nullptr,nullptr,nullptr,napi_default,nullptr}
    };
    napi_define_properties(env,exports,sizeof(props)/sizeof(props[0]),props);
    napi_value value;OH_NativeXComponent* c=nullptr;
    if(napi_get_named_property(env,exports,OH_NATIVE_XCOMPONENT_OBJ,&value)==napi_ok&&napi_unwrap(env,value,reinterpret_cast<void**>(&c))==napi_ok&&c) {
        OH_LOG_Print(LOG_APP, LOG_INFO, 0xD001, "PhotoCraft", "Registering XComponent callbacks");
        OH_NativeXComponent_RegisterCallback(c,&callbacks);
        OH_NativeXComponent_RegisterMouseEventCallback(c,&mouseCallbacks);
        OH_NativeXComponent_RegisterKeyEventCallbackWithResult(c,Key);
        OH_NativeXComponent_RegisterBlurEventCallback(c,Blur);
        OH_NativeXComponent_RegisterUIInputEventCallback(c,Axis,ARKUI_UIINPUTEVENT_TYPE_AXIS);
    }
    napi_add_env_cleanup_hook(env,Cleanup,nullptr);
    return exports;
}
napi_module module{1,0,nullptr,Init,"photocraft",nullptr,{0}};
}
extern "C" void pc_request_vsync() {
    if(vsync&&attached&&!framePending.exchange(true)) {
        if(OH_NativeVSync_RequestFrame(vsync,Vsync,nullptr)!=0)framePending=false;
    }
}
extern "C" __attribute__((constructor)) void RegisterPhotoCraft(){napi_module_register(&module);}
