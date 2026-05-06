import * as THREE from 'three'
import { VRMLoaderPlugin, VRMUtils, type VRM } from '@pixiv/three-vrm'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'

export interface AvatarBounds {
  x: number
  y: number
  width: number
  height: number
}

/**
 * VRM 角色运行时：Three.js 渲染 + @pixiv/three-vrm 加载/动画。
 *
 * 设计点：
 * - 透明背景（`alpha: true` + clearColor alpha 0），配合 Tauri 透明窗口
 * - 相机看向胸口高度（1.3m），桌宠视角偏半身
 * - VRMUtils.rotateVRM0 兼容 VRM 0.x（自动旋转 180° 面向相机）；VRM 1.0 是 no-op
 * - 自带 RAF 渲染循环，vrm.update(dt) 驱动 spring bone（头发飘动等）
 *
 * 注：getBounds() 当前未被消费（M1 spike 期 hitbox 上报推到后续 task），保留以便复用。
 */
export class VRMRuntime {
  /** 呼吸角速度：4 秒/周期，2π/4 ≈ 1.5708 rad/s。注释里的"频率"以人类静息呼吸 12-15 次/分钟为参考。 */
  private static readonly BREATH_RAD_PER_SEC = (2 * Math.PI) / 4

  private renderer: THREE.WebGLRenderer | null = null
  private scene: THREE.Scene | null = null
  private camera: THREE.PerspectiveCamera | null = null
  private vrm: VRM | null = null
  private lastFrameMs: number | null = null
  private breathPhase = 0
  private rafId: number | null = null

  init(canvas: HTMLCanvasElement): void {
    const w = canvas.clientWidth || 320
    const h = canvas.clientHeight || 320

    this.renderer = new THREE.WebGLRenderer({
      canvas,
      alpha: true,
      antialias: true,
    })
    this.renderer.setPixelRatio(window.devicePixelRatio || 1)
    this.renderer.setSize(w, h, false)
    this.renderer.setClearColor(0x000000, 0)

    this.scene = new THREE.Scene()

    this.camera = new THREE.PerspectiveCamera(30, w / h, 0.1, 20)
    this.camera.position.set(0, 1.3, 1.5)
    this.camera.lookAt(0, 1.3, 0)

    const dirLight = new THREE.DirectionalLight(0xffffff, 1.2)
    dirLight.position.set(1, 2, 1)
    this.scene.add(dirLight)
    this.scene.add(new THREE.AmbientLight(0xffffff, 0.6))
  }

  async loadModel(url: string): Promise<void> {
    if (!this.scene || !this.renderer || !this.camera) {
      throw new Error('VRM runtime not initialized')
    }

    const loader = new GLTFLoader()
    loader.register((parser) => new VRMLoaderPlugin(parser))

    const gltf = await loader.loadAsync(url)

    // 异步加载期间实例可能已被 destroy（HMR 热替换 / 用户关窗口），
    // 静默放弃，避免 `null.add(...)` 崩溃。
    if (!this.scene) return

    const vrm: VRM | undefined = gltf.userData.vrm
    if (!vrm) {
      throw new Error('Loaded GLTF does not contain VRM data — 确认文件是 .vrm 而非普通 .glb')
    }

    // VRM 0.x 默认朝 +Z（背对相机），需旋转；VRM 1.0 朝 -Z 不需要。该工具自动判断。
    VRMUtils.rotateVRM0(vrm)

    // 性能小开关：关掉 frustumCulled，桌宠 canvas 尺寸小不必裁剪
    vrm.scene.traverse((obj) => {
      obj.frustumCulled = false
    })

    // T-pose → A-pose：把双臂从水平旋转到自然下垂角度
    this.applyIdlePose(vrm)

    this.scene.add(vrm.scene)
    this.vrm = vrm

    this.startLoop()
  }

  /**
   * 把 T-pose 调整到自然待机姿势：上臂沿身体下垂（±70°），前臂略内收。
   * 用 normalized bone（VRM 标准 humanoid 骨骼，跨模型一致）。
   */
  private applyIdlePose(vrm: VRM): void {
    const h = vrm.humanoid
    if (!h) return
    const lUpper = h.getNormalizedBoneNode('leftUpperArm')
    const rUpper = h.getNormalizedBoneNode('rightUpperArm')
    const lLower = h.getNormalizedBoneNode('leftLowerArm')
    const rLower = h.getNormalizedBoneNode('rightLowerArm')
    if (lUpper) lUpper.rotation.z = 1.2 // ~70°
    if (rUpper) rUpper.rotation.z = -1.2
    if (lLower) lLower.rotation.y = 0.2 // 前臂略内收
    if (rLower) rLower.rotation.y = -0.2
  }

  private startLoop(): void {
    const tick = () => {
      this.rafId = requestAnimationFrame(tick)
      const now = performance.now()
      const dt = this.lastFrameMs === null ? 0 : (now - this.lastFrameMs) / 1000
      this.lastFrameMs = now
      if (this.vrm) {
        this.vrm.update(dt)
        this.applyBreathing(dt)
      }
      if (this.renderer && this.scene && this.camera) {
        this.renderer.render(this.scene, this.camera)
      }
    }
    tick()
  }

  /**
   * 呼吸感：upperChest / chest / spine 在 X 轴做极小幅度摆动（±1.4°），
   * 周期 4 秒（成人静息呼吸频率 12-15 次/分钟）。
   */
  private applyBreathing(dt: number): void {
    if (!this.vrm?.humanoid) return
    this.breathPhase += dt * VRMRuntime.BREATH_RAD_PER_SEC
    const h = this.vrm.humanoid
    const chest =
      h.getNormalizedBoneNode('upperChest') ??
      h.getNormalizedBoneNode('chest') ??
      h.getNormalizedBoneNode('spine')
    if (chest) {
      chest.rotation.x = 0.025 * Math.sin(this.breathPhase)
    }
  }

  /** 把 VRM 3D bbox 投影到 canvas 像素空间，给 Tauri hitbox 用 */
  getBounds(): AvatarBounds | null {
    if (!this.vrm || !this.camera || !this.renderer) return null

    const box = new THREE.Box3().setFromObject(this.vrm.scene)
    if (!isFinite(box.min.x)) return null

    const corners: [number, number, number][] = [
      [box.min.x, box.min.y, box.min.z],
      [box.max.x, box.min.y, box.min.z],
      [box.min.x, box.max.y, box.min.z],
      [box.max.x, box.max.y, box.min.z],
      [box.min.x, box.min.y, box.max.z],
      [box.max.x, box.min.y, box.max.z],
      [box.min.x, box.max.y, box.max.z],
      [box.max.x, box.max.y, box.max.z],
    ]

    const sz = this.renderer.getSize(new THREE.Vector2())
    let minX = Infinity
    let minY = Infinity
    let maxX = -Infinity
    let maxY = -Infinity
    const v = new THREE.Vector3()

    for (const [x, y, z] of corners) {
      v.set(x, y, z).project(this.camera)
      const sx = (v.x + 1) * 0.5 * sz.x
      const sy = (1 - v.y) * 0.5 * sz.y
      if (sx < minX) minX = sx
      if (sy < minY) minY = sy
      if (sx > maxX) maxX = sx
      if (sy > maxY) maxY = sy
    }

    // 裁剪到 canvas 内
    minX = Math.max(0, minX)
    minY = Math.max(0, minY)
    maxX = Math.min(sz.x, maxX)
    maxY = Math.min(sz.y, maxY)

    const width = maxX - minX
    const height = maxY - minY
    if (!isFinite(width) || !isFinite(height) || width <= 0 || height <= 0) {
      return null
    }

    return { x: minX, y: minY, width, height }
  }

  destroy(): void {
    if (this.rafId !== null) {
      cancelAnimationFrame(this.rafId)
      this.rafId = null
    }
    if (this.vrm && this.scene) {
      this.scene.remove(this.vrm.scene)
      VRMUtils.deepDispose(this.vrm.scene)
      this.vrm = null
    }
    this.renderer?.dispose()
    this.renderer = null
    this.scene = null
    this.camera = null
  }
}
