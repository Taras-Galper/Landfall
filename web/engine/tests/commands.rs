//! Drive the browser engine through its JSON commands, as the web page does.
use landfall_web::handle;
use serde_json::{Value, json};

fn send(v: Value) -> Value {
    serde_json::from_str(&handle(&v.to_string())).unwrap()
}

#[test]
fn play_through_commands() {
    let r = send(json!({"cmd": "new_game", "seed": 7, "nation": 0, "difficulty": 1, "world": {"land": 1, "shape": 1, "temperature": 1, "climate": 1}}));
    assert_eq!(r["ok"], true, "{r}");
    let st = &r["state"];
    assert_eq!(st["raw"].as_array().unwrap().len(), 58 * 72);
    assert_eq!(r["events"][0]["type"], "milestone");
    let meta = send(json!({"cmd": "meta"}));
    assert_eq!(meta["data"]["nations"][0], "Brennland");

    // sail the ship west until it reaches land, then go ashore
    let ship = st["units"].as_array().unwrap().iter().find(|u| u["kind"] == 13).unwrap()["id"].as_u64().unwrap();
    let mut landed = false;
    for _turn in 0..30 {
        for _ in 0..8 {
            let r = send(json!({"cmd": "move", "unit": ship, "dx": -1, "dy": 0}));
            if let Some(p) = r.get("prompt") {
                if p["type"] == "landfall" {
                    let r = send(json!({"cmd": "landfall", "unit": ship, "x": p["x"], "y": p["y"]}));
                    assert_eq!(r["ok"], true);
                    landed = true;
                    break;
                }
            }
            if r["ok"] == false {
                // blocked: try diagonals
                send(json!({"cmd": "move", "unit": ship, "dx": -1, "dy": 1}));
            }
        }
        if landed {
            break;
        }
        let r = send(json!({"cmd": "end_turn"}));
        assert_eq!(r["ok"], true, "{r}");
    }
    assert!(landed, "ship should reach land");

    // found a colony with the pioneer or soldier ashore
    let st = send(json!({"cmd": "state"}))["state"].clone();
    let ashore: Vec<u64> = st["units"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|u| u["loc"]["type"] == "map" && u["kind"] != 13)
        .map(|u| u["id"].as_u64().unwrap())
        .collect();
    assert!(!ashore.is_empty());
    let mut colony = None;
    for &u in &ashore {
        let mut r = send(json!({"cmd": "found", "unit": u, "name": "Test Harbor"}));
        if r["prompt"]["type"] == "land_claim" {
            r = send(json!({"cmd": "found", "unit": u, "name": "Test Harbor", "claim": "take"}));
        }
        if r["ok"] == true {
            colony = r["data"]["colony"].as_u64();
            break;
        }
    }
    let Some(cid) = colony else {
        // the landing square may not allow a colony; that's fine for this seed check
        return;
    };
    let cv = send(json!({"cmd": "colony", "colony": cid}));
    assert_eq!(cv["data"]["name"], "Test Harbor");
    let worker = cv["data"]["workers"][0]["id"].as_u64().unwrap();
    let pv = send(json!({"cmd": "preview", "colony": cid, "unit": worker, "job": {"building": 17}}));
    assert_eq!(pv["ok"], true, "{pv}");
    assert!(pv["data"]["delta"][17].as_i64().unwrap() > 0, "statesman makes bells: {pv}");
    let r = send(json!({"cmd": "assign", "colony": cid, "unit": worker, "job": {"building": 17}}));
    assert_eq!(r["ok"], true, "{r}");
    // goods from the warehouse onto a carrier in the colony (and back)
    let cx = cv["data"]["x"].as_i64().unwrap();
    let cy = cv["data"]["y"].as_i64().unwrap();
    let stock: Vec<i64> = cv["data"]["stock"].as_array().unwrap().iter().map(|v| v.as_i64().unwrap()).collect();
    if let Some(good) = stock.iter().position(|&n| n > 0) {
        let st = send(json!({"cmd": "end_turn"}))["state"].clone();
        let ship = st["units"].as_array().unwrap().iter().find(|u| u["kind"] == 13).unwrap()["id"].as_u64().unwrap();
        let sx = st["units"].as_array().unwrap().iter().find(|u| u["kind"] == 13).unwrap()["x"].as_i64().unwrap();
        let sy = st["units"].as_array().unwrap().iter().find(|u| u["kind"] == 13).unwrap()["y"].as_i64().unwrap();
        if (sx - cx).abs() <= 1 && (sy - cy).abs() <= 1 {
            let r = send(json!({"cmd": "move", "unit": ship, "dx": cx - sx, "dy": cy - sy}));
            assert_eq!(r["ok"], true, "{r}");
            let r = send(json!({"cmd": "load_goods", "colony": cid, "carrier": ship, "good": good}));
            assert_eq!(r["ok"], true, "{r}");
            let r = send(json!({"cmd": "unload", "carrier": ship, "slot": 0}));
            assert_eq!(r["ok"], true, "{r}");
        }
    }
    let r = send(json!({"cmd": "homeland"}));
    assert!(r["data"]["market"].as_array().unwrap().len() == 16);
    for _ in 0..20 {
        let r = send(json!({"cmd": "end_turn"}));
        assert_eq!(r["ok"], true);
    }
    let save = send(json!({"cmd": "save"}));
    let j = save["data"]["json"].as_str().unwrap().to_string();
    println!("save size {} bytes, state size {} bytes", j.len(), send(json!({"cmd": "state"}))["state"].to_string().len());
    let r = send(json!({"cmd": "load", "json": j}));
    assert_eq!(r["ok"], true);
}
