/* 設計図の操作を、WRAC の音のエンジンへ渡す。ブラウザ単体では invoke が無いので見た目だけ動く。 */
(() => {
  const prefixes = {
    "Oscillator A": "o1_", "Oscillator B": "o2_", "Oscillator C": "o3_", "Oscillator D": "o4_",
    "Filter A1": "f1_", "Filter B1": "f2_", "Filter C1": "f3_", "Filter D1": "f4_",
    "Filter Env A1": "fe1_", "Filter Env B1": "fe2_", "Filter Env C1": "fe3_", "Filter Env D1": "fe4_",
    "Amp Env A": "a1_", "Amp Env B": "a2_", "Amp Env C": "a3_", "Amp Env D": "a4_",
    "Mod Env A": "me1_", "Mod Env B": "me2_", "Mod Env C": "me3_", "Mod Env D": "me4_",
    "LFO A": "l1_", "LFO B": "l2_", "LFO C": "l3_", "LFO D": "l4_",
    "Master": "master_"
  };
  const names = {
    on: "on", octave: "octave", note: "note", fine: "fine", volume: "volume", pan: "pan",
    stereo: "stereo", wave: "wave", phase: "phase", inv: "inv", retrigger: "retrigger",
    voices: "voices", detune: "detune", type: "type", cutoff: "cutoff", resonance: "resonance",
    drive: "drive", amount: "amount", attack: "attack", attackcurve: "attack_curve",
    decay: "decay", decaycurve: "decay_curve", sustain: "sustain", sustaincurve: "sustain_curve",
    release: "release", releasecurve: "release_curve", sync: "sync", target: "target",
    chain: "chain", tempo: "tempo", rate: "rate", transpose: "transpose",
    bendrange: "bend", mode: "mode", monolegato: "legato", portamento: "portamento",
    polyphony: "polyphony", mastervolume: "volume"
  };
  const links = { Sync: "sync", B: "link_b", C: "link_c", D: "link_d" };
  const key = (text) => text.toLowerCase().replace(/[^a-z0-9]/g, "");
  const paramId = (section, label) => {
    const title = section.querySelector(":scope > h2")?.textContent.trim() ?? "";
    const prefix = prefixes[title];
    const name = names[key(label)];
    return prefix && name ? prefix + name : null;
  };
  const send = (id, value) => {
    const invoke = window.invoke;
    if (typeof invoke === "function" && id) void invoke("set_mote_param", { id, value });
  };
  const angleOf = (knob) => {
    const angle = parseFloat(getComputedStyle(knob).getPropertyValue("--a"));
    return Number.isFinite(angle) ? angle : 0;
  };
  const unitOf = (knob) => Math.min(1, Math.max(0, (angleOf(knob) + 135) / 270));
  const setAngle = (knob, angle) => {
    const next = Math.min(135, Math.max(-135, angle));
    knob.style.setProperty("--a", next.toFixed(1) + "deg");
    return unitOf(knob);
  };

  document.querySelectorAll(".panel").forEach((section) => {
    section.querySelectorAll(".ctl").forEach((control) => {
      const label = control.querySelector(":scope > span")?.textContent ?? "";
      const id = paramId(section, label);
      const knob = control.querySelector(".k");
      if (knob && id) {
        knob.addEventListener("pointerdown", (event) => {
          if (event.button !== 0) return;
          knob.setPointerCapture(event.pointerId);
          const startY = event.clientY;
          const start = angleOf(knob);
          const move = (ev) => send(id, setAngle(knob, start + (startY - ev.clientY) * 1.5));
          const up = () => {
            knob.removeEventListener("pointermove", move);
            knob.removeEventListener("pointerup", up);
          };
          knob.addEventListener("pointermove", move);
          knob.addEventListener("pointerup", up);
        });
      }
      const toggle = control.querySelector(":scope > .t");
      if (toggle && id) {
        toggle.addEventListener("click", () => {
          const on = toggle.classList.toggle("on");
          send(id, on ? 1 : 0);
        });
      }
      const select = control.querySelector("select");
      if (select && id) {
        select.addEventListener("change", () => {
          const count = Math.max(1, select.options.length - 1);
          send(id, select.selectedIndex / count);
        });
      }
      const seg = control.querySelector(".seg");
      if (seg && id) {
        const choices = [...seg.querySelectorAll(":scope > span")];
        choices.forEach((choice, index) => {
          choice.addEventListener("click", () => {
            choices.forEach((other, otherIndex) => other.classList.toggle("on", otherIndex === index));
            const count = Math.max(1, choices.length - 1);
            send(id, index / count);
          });
        });
      }
    });
    section.querySelectorAll(".syncs .sync").forEach((link) => {
      const name = links[link.textContent.trim()];
      const title = section.querySelector(":scope > h2")?.textContent.trim() ?? "";
      const prefix = prefixes[title];
      const id = prefix && name ? prefix + name : null;
      if (!id) return;
      link.addEventListener("click", () => {
        const on = link.classList.toggle("on");
        send(id, on ? 1 : 0);
      });
    });
  });

  const publish = () => {
    document.querySelectorAll(".panel").forEach((section) => {
      section.querySelectorAll(".ctl").forEach((control) => {
        const label = control.querySelector(":scope > span")?.textContent ?? "";
        const id = paramId(section, label);
        if (!id) return;
        const knob = control.querySelector(".k");
        if (knob) send(id, unitOf(knob));
        const toggle = control.querySelector(":scope > .t");
        if (toggle) send(id, toggle.classList.contains("on") ? 1 : 0);
        const select = control.querySelector("select");
        if (select) {
          const count = Math.max(1, select.options.length - 1);
          send(id, select.selectedIndex / count);
        }
        const seg = control.querySelector(".seg");
        if (seg) {
          const choices = [...seg.querySelectorAll(":scope > span")];
          const index = Math.max(0, choices.findIndex((choice) => choice.classList.contains("on")));
          const count = Math.max(1, choices.length - 1);
          send(id, index / count);
        }
      });
      section.querySelectorAll(".syncs .sync").forEach((link) => {
        const name = links[link.textContent.trim()];
        const title = section.querySelector(":scope > h2")?.textContent.trim() ?? "";
        const prefix = prefixes[title];
        if (prefix && name) send(prefix + name, link.classList.contains("on") ? 1 : 0);
      });
    });
  };
  const boot = () => {
    if (typeof window.invoke !== "function") return false;
    publish();
    return true;
  };
  if (!boot()) {
    let tries = 0;
    const timer = setInterval(() => {
      tries += 1;
      if (boot() || tries > 20) clearInterval(timer);
    }, 100);
  }

  const notes = { a: 60, w: 61, s: 62, e: 63, d: 64, f: 65, t: 66, g: 67, y: 68, h: 69, u: 70, j: 71, k: 72 };
  const held = new Set();
  addEventListener("keydown", (event) => {
    if (event.repeat || event.metaKey || event.ctrlKey || event.altKey) return;
    if (event.target.closest("select, input, textarea")) return;
    const note = notes[event.key.toLowerCase()];
    if (note === undefined || held.has(note)) return;
    held.add(note);
    const invoke = window.invoke;
    if (typeof invoke === "function") void invoke("note_on", { key: note, velocity: 0.8 });
  });
  addEventListener("keyup", (event) => {
    const note = notes[event.key.toLowerCase()];
    if (note === undefined) return;
    held.delete(note);
    const invoke = window.invoke;
    if (typeof invoke === "function") void invoke("note_off", { key: note });
  });
})();
