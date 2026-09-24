import * as THREE from 'three';
// Procedural clips for the Mixamo rig. Poses are written as world-space limb
// directions (character faces +Z, up +Y, its left is +X), sampled to keyframes.
const FPS = 30;
const _q = new THREE.Quaternion(), _p = new THREE.Quaternion(), _a = new THREE.Vector3(), _b = new THREE.Vector3();
const V = (x, y, z) => new THREE.Vector3(x, y, z).normalize();

export function buildClips(root) {
  if (arguments[1]) { const m = new THREE.AnimationMixer(root); m.clipAction(arguments[1]).play(); m.setTime(0); const snap = []; root.traverse(o => snap.push([o, o.position.clone(), o.quaternion.clone(), o.scale.clone()])); m.stopAllAction(); m.uncacheRoot(root); for (const [o, p, q, sc] of snap) { o.position.copy(p); o.quaternion.copy(q); o.scale.copy(sc); } root.updateMatrixWorld(true); }
  const bones = {};
  root.traverse(o => { if (o.isBone) { const m = o.name.match(/mixamorig:?([A-Za-z0-9]+?)_\d+$/); if (m) bones[m[1]] = o; } });
  const rest = new Map(); for (const b of Object.values(bones)) rest.set(b, { q: b.quaternion.clone(), p: b.position.clone() });
  const hips = bones.Hips;
  const H = new THREE.Box3().setFromObject(root).getSize(new THREE.Vector3()).y;

  const reset = () => { for (const [b, r] of rest) { b.quaternion.copy(r.q); b.position.copy(r.p); } root.updateMatrixWorld(true); };
  const setWorld = (bone, D) => { // world rotation W' = D * W
    bone.getWorldQuaternion(_q); bone.parent.getWorldQuaternion(_p);
    bone.quaternion.copy(_p.invert().multiply(D.multiply(_q))); bone.updateMatrixWorld(true);
  };
  const rot = (name, axis, ang) => setWorld(bones[name], new THREE.Quaternion().setFromAxisAngle(axis, ang));
  const aim = (name, childName, dir) => {
    const b = bones[name], c = bones[childName];
    b.getWorldPosition(_a); c.getWorldPosition(_b);
    setWorld(b, new THREE.Quaternion().setFromUnitVectors(_b.sub(_a).normalize(), dir.clone().normalize()));
  };
  const lift = dy => { hips.getWorldPosition(_a); _a.y += dy * H; hips.position.copy(hips.parent.worldToLocal(_a)); hips.updateMatrixWorld(true); };
  const X = new THREE.Vector3(1, 0, 0), Y = new THREE.Vector3(0, 1, 0), Z = new THREE.Vector3(0, 0, 1);
  const armsDown = (sw = 0, spread = 0.22, bend = 0.35) => {
    for (const [s, sx] of [['Left', 1], ['Right', -1]]) {
      const a = sw * sx;
      aim(s + 'Arm', s + 'ForeArm', V(spread * sx, -Math.cos(a), Math.sin(a)));
      aim(s + 'ForeArm', s + 'Hand', V(spread * 0.6 * sx, -Math.cos(a + bend), Math.sin(a + bend)));
      aim(s + 'Hand', s + 'HandIndex1', V(spread * 0.4 * sx, -Math.cos(a + bend * 1.2), Math.sin(a + bend * 1.2)));
    }
  };

  const poses = {
    Idle: { dur: 3, fn(t) { const w = 2 * Math.PI * t / 3;
      lift(-0.004 + 0.004 * Math.sin(w * 2));
      rot('Spine', X, 0.07 + 0.02 * Math.sin(w)); rot('Spine2', X, 0.05 + 0.025 * Math.sin(w));
      rot('Neck', X, 0.12); rot('Head', Z, 0.22 * Math.sin(w) ); rot('Head', X, 0.05 * Math.sin(w * 2));
      armsDown(0.06 * Math.sin(w), 0.12 + 0.03 * Math.sin(w), 0.3);
      for (const [s, sx] of [['Left', 1], ['Right', -1]]) aim(s + 'UpLeg', s + 'Leg', V(0.06 * sx, -1, 0));
    } },
    Walk: { dur: 1.2, fn(t) { const w = 2 * Math.PI * t / 1.2, s = Math.sin(w);
      lift(-0.02 + 0.012 * Math.cos(2 * w));
      rot('Hips', Y, 0.14 * s); rot('Hips', Z, 0.04 * s);
      rot('Spine', X, 0.14); rot('Spine2', Y, -0.2 * s);
      rot('Neck', X, 0.1); rot('Head', Z, 0.12 + 0.05 * Math.sin(2 * w)); rot('Head', Y, 0.06 * s);
      for (const [side, sx, ph] of [['Left', 1, 0], ['Right', -1, Math.PI]]) {
        const a = 0.5 * Math.sin(w + ph), knee = 0.08 + 0.85 * Math.max(0, Math.cos(w + ph)) ** 1.5;
        aim(side + 'UpLeg', side + 'Leg', V(0.05 * sx, -Math.cos(a), Math.sin(a)));
        aim(side + 'Leg', side + 'Foot', V(0.02 * sx, -Math.cos(a - knee), Math.sin(a - knee)));
        aim(side + 'Foot', side + 'ToeBase', V(0, -0.35 + 0.4 * Math.max(0, -Math.sin(w + ph + 0.8)), 1));
      }
      armsDown(-0.35 * s, 0.1, 0.35); // arms swing opposite to legs (left arm ~ right leg)
    } },
    Wave: { dur: 2, fn(t) { const w = 2 * Math.PI * t / 2, s = Math.sin(2 * w);
      lift(-0.004);
      rot('Spine', X, 0.05); rot('Spine2', Z, 0.06);
      rot('Neck', X, 0.1); rot('Head', Z, -0.25 + 0.06 * Math.sin(w)); 
      armsDown(0.03 * Math.sin(w), 0.12, 0.3);
      aim('RightArm', 'RightForeArm', V(-0.85, 0.45, 0.25));
      aim('RightForeArm', 'RightHand', V(-0.1 - 0.45 * s, 1, 0.15));
      aim('RightHand', 'RightHandIndex1', V(-0.05 - 0.6 * s, 1, 0.1));
      for (const [s2, sx] of [['Left', 1], ['Right', -1]]) aim(s2 + 'UpLeg', s2 + 'Leg', V(0.06 * sx, -1, 0));
    } },
  };

  const clips = [];
  for (const [name, { dur, fn }] of Object.entries(poses)) {
    const n = Math.round(dur * FPS) + 1, times = [], q = new Map(), hp = [];
    for (let i = 0; i < n; i++) {
      const t = (i % (n - 1)) / FPS; // last key == first key: seamless loop
      reset(); fn(t); times.push(i / FPS);
      for (const b of rest.keys()) { if (!q.has(b)) q.set(b, []); q.get(b).push(...b.quaternion.toArray()); }
      hp.push(...hips.position.toArray());
    }
    const tracks = [...q].map(([b, v]) => new THREE.QuaternionKeyframeTrack(b.name + '.quaternion', times, v));
    tracks.push(new THREE.VectorKeyframeTrack(hips.name + '.position', times, hp));
    clips.push(new THREE.AnimationClip(name, -1, tracks));
  }
  reset();
  return clips;
}
