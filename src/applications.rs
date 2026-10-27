use serde_json::Value;
pub fn allowed(payload: &Value, kind: &str) -> bool {
    let scope=payload.get("application_access").or_else(||payload.get("features").and_then(|v|v.get("application_access")));
    let scope=match scope {None=>"both",Some(v)=>match v.as_str(){Some(s)=>s,None=>return false}};
    let bound=payload.get("application_type").filter(|v|!v.is_null());
    matches!(kind,"desktop"|"web")&&matches!(scope,"both"|"desktop"|"web")&&(scope=="both"||scope==kind)&&match bound{None=>scope=="both",Some(v)=>v.as_str()==Some(kind)}
}
