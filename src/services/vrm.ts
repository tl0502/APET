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
 * 角色取景模式：
 * - half：胸口以上（默认）—— 320×320 等比窗口；适合对话/表情/眨眼场景，细节最易感知
 * - full：全身 —— 推荐 1:1.6 长宽（如 320×512）；为 M2+ 装扮 / 多动作预留视角
 *
 * 切换成本：仅相机 position + 默认 lookAt 平视点联动；模型本身不变。
 * 窗口实际尺寸由 PetCanvas/调用方决定，本类只负责相机布景。
 */
export type AvatarView = 'half' | 'full'

/**
 * VRMRuntime.init 选项。
 * - `preserveDrawingBuffer`：构造期开 WebGL `preserveDrawingBuffer`，让 captureSnapshot
 *   能可靠在 render 后任意时刻 toDataURL。**只用于一次性截图场景**（#26 VRM 头像导出）；
 *   live 桌宠不开（避免不必要的 buffer 持有 + 轻微性能损耗）。
 */
export interface VRMRuntimeInitOptions {
  preserveDrawingBuffer?: boolean
}

// #29 桌宠反应动作 ID 契约。#23 接 reaction_table 时扩这个 union。
export type PetActionId =
  | 'glance_up' | 'glance_down'  // 2026-05-25: reminder placement direction → pet glance（spec 2026-05-25-pet-reminder-card-stack §5）
  | 'nod'                        // #29 实现（保留兼容，行为 = glance_up）
  | 'head_pat' | 'surprised' | 'fall_asleep' | 'dizzy' | 'protest' | 'cheer'  // #23 placeholder
  | 'drink' | 'stretch' | 'sleep' | 'wander' | 'idle'                          // #23 placeholder

interface ViewConfig {
  /** 相机世界坐标 */
  cameraPos: THREE.Vector3
  /** 相机 lookAt 的中心点（一般是角色脊柱中部） */
  cameraTarget: THREE.Vector3
}

const VIEW_CONFIGS: Record<AvatarView, ViewConfig> = {
  // 半身：相机看胸口 1.3m，距离 1.5；FOV 30° 下覆盖头到腹部
  half: {
    cameraPos: new THREE.Vector3(0, 1.3, 1.5),
    cameraTarget: new THREE.Vector3(0, 1.3, 0),
  },
  // 全身：相机看腰部 0.8m，距离 3.2；FOV 30° 下垂直视野 = 2 × 3.2 × tan(15°) ≈ 1.72m，
  // 覆盖 -0.06~1.66m，标准 VRM (≈1.6m) 头脚均在画面内各留 ≈6cm 余量。
  // （旧值距离 2.8 → 覆盖 0.05~1.55m，会切脚）
  full: {
    cameraPos: new THREE.Vector3(0, 0.8, 3.2),
    cameraTarget: new THREE.Vector3(0, 0.8, 0),
  },
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
  /** 一次眨眼总时长（含闭眼 + 睁眼）：150ms，参考人类自然眨眼 100-400ms 偏快档。 */
  private static readonly BLINK_DURATION_S = 0.15
  /** 眨眼间隔下限（秒）：成人静息状态平均 15-20 次/分钟，对应间隔 3-4s；放宽到 4s 避免单调。 */
  private static readonly BLINK_INTERVAL_MIN_S = 4
  private static readonly BLINK_INTERVAL_MAX_S = 8
  /**
   * lookAt 平滑时间常数（秒）：每经过 τ 秒，目标到当前位置的距离衰减到 1/e（≈37%）。
   *
   * τ=0.1s 等价于原版"60fps 下 α=0.15/帧"的指数衰减速度（数学上 0.85^60 ≈ exp(-1/0.1006)），
   * 但帧率无关：30fps 下跟随时间不会翻倍。每帧 α = 1 - exp(-dt/τ)。
   */
  private static readonly LOOK_AT_TAU_S = 0.1

  private renderer: THREE.WebGLRenderer | null = null
  private scene: THREE.Scene | null = null
  private camera: THREE.PerspectiveCamera | null = null
  private vrm: VRM | null = null
  /** playGlance 动画进行中标志：防止并发 glance/nod 导致 baseX 捕获中间值、动画结束后 rotation 卡位。 */
  private _glanceInProgress = false
  private lookAtTarget: THREE.Object3D | null = null
  private lastFrameMs: number | null = null
  private breathPhase = 0
  /** 距离下次眨眼的倒计时（秒）；≤ 0 时触发，进入 blinkProgress 阶段。 */
  private blinkCountdown = 0
  /** 当前眨眼进度（秒，0 ≤ p < BLINK_DURATION_S）；< 0 表示不在眨眼中。 */
  private blinkProgress = -1
  /** 用户光标在 canvas NDC 空间 [-1,1] 的位置；null 表示不跟随（视线回中）。 */
  private cursorNdc: { x: number; y: number } | null = null
  /**
   * lookAt 目标在**相机本地坐标**下的平滑位置。
   * 跟官方示例对齐：lookAtTarget 是 camera 的子节点，position=(0,0,0) 即"看相机自己 = 看用户"。
   * 鼠标在 canvas 内偏移 → 这个本地位置 x/y 跟着偏移，角色眼神就跟过来。
   */
  private lookAtSmoothed = new THREE.Vector3()
  /** 复用的 desired 缓冲，避免 RAF 热路径每帧 new Vector3() 触发 GC 压力。 */
  private lookAtDesired = new THREE.Vector3()
  /** 当前取景模式；init() 时确定，运行期通过 setView() 切换。 */
  private view: AvatarView = 'half'
  private rafId: number | null = null
  /** #26 头像导出：camera 距离倍率（1.0 = 默认；< 1 拉近放大头部；> 1 拉远收缩）。
   *  setCameraZoom() 修改；snapshot 流程实时反映到 camera.position。 */
  private cameraZoom = 1
  /** #26 头像导出：camera lookAt 中心相对默认值的偏移（米）。
   *  +y 让视线焦点上移（拍头），+x 让画面右移（人在左）。 */
  private cameraPan = { x: 0, y: 0 }

  init(canvas: HTMLCanvasElement, view: AvatarView = 'half', opts?: VRMRuntimeInitOptions): void {
    const w = canvas.clientWidth || 320
    const h = canvas.clientHeight || 320

    this.view = view
    const cfg = VIEW_CONFIGS[view]

    this.renderer = new THREE.WebGLRenderer({
      canvas,
      alpha: true,
      antialias: true,
      preserveDrawingBuffer: opts?.preserveDrawingBuffer ?? false,
    })
    this.renderer.setPixelRatio(window.devicePixelRatio || 1)
    this.renderer.setSize(w, h, false)
    this.renderer.setClearColor(0x000000, 0)

    this.scene = new THREE.Scene()

    this.camera = new THREE.PerspectiveCamera(30, w / h, 0.1, 20)
    this.camera.position.copy(cfg.cameraPos)
    this.camera.lookAt(cfg.cameraTarget)
    // lookAtSmoothed 是相机本地坐标，(0,0,0) = 直视用户；不依赖视角，切视角无需重置
    this.lookAtSmoothed.set(0, 0, 0)

    const dirLight = new THREE.DirectionalLight(0xffffff, 1.2)
    dirLight.position.set(1, 2, 1)
    this.scene.add(dirLight)
    this.scene.add(new THREE.AmbientLight(0xffffff, 0.6))
  }

  /**
   * 运行期切换取景模式。仅平移相机 + 重置 lookAt 平滑位，不重载模型。
   * 调用方通常同时改 canvas / 窗口尺寸（aspect ratio 联动）。
   */
  setView(view: AvatarView): void {
    if (!this.camera || this.view === view) return
    this.view = view
    this.applyCameraTransform()
    // lookAtSmoothed 是相机本地坐标，切视角不影响"看用户"的语义；保持原值即可
  }

  /**
   * #26 头像导出：camera 拉近/拉远（zoom > 1 拉远，< 1 拉近）。
   * 调用方传 0.5-2.0 区间最自然。0.6 ≈ 头部特写，1.0 默认胸口以上，1.5 ≈ 半身。
   * 立即生效（不平滑），用于实时预览滑块。
   */
  setCameraZoom(zoom: number): void {
    if (!this.camera || !Number.isFinite(zoom) || zoom <= 0) return
    this.cameraZoom = zoom
    this.applyCameraTransform()
  }

  /**
   * #26 头像导出：camera lookAt 中心偏移（米）。
   * +y 把视线焦点抬高（拍头部），-y 压低；±x 左右偏移。立即生效。
   * 范围建议 ±0.3m（再大就出了角色身体）。
   */
  setCameraPan(x: number, y: number): void {
    if (!this.camera || !Number.isFinite(x) || !Number.isFinite(y)) return
    this.cameraPan = { x, y }
    this.applyCameraTransform()
  }

  /** 把 view + zoom + pan 三个状态综合算出最终 camera position + lookAt 并应用。 */
  private applyCameraTransform(): void {
    if (!this.camera) return
    const cfg = VIEW_CONFIGS[this.view]
    // zoom 调 z 距离 + 同步缩 lookAt 偏移度：cameraPos.z * zoom 直观映射"远近"
    const pos = cfg.cameraPos.clone()
    pos.z *= this.cameraZoom
    // pan 相对 lookAt 中心偏移：camera 也跟着偏移让"看的点"对齐，避免视角穿模
    pos.x += this.cameraPan.x
    pos.y += this.cameraPan.y
    const target = cfg.cameraTarget.clone()
    target.x += this.cameraPan.x
    target.y += this.cameraPan.y
    this.camera.position.copy(pos)
    this.camera.lookAt(target)
  }

  /**
   * #26 头像导出：设 VRM 表情预设（neutral/happy/angry/sad/relaxed/surprised）。
   *
   * VRM 1.0 标准 emotion 集合（[VRMC_vrm-1.0 expressions.md]）；VRM 0.x 模型对应映射由 three-vrm
   * 内部完成（'happy' ≈ 'joy'，'sad' ≈ 'sorrow'）。setValue 是累加权重模式，本方法
   * 先清除其它 emotion 再设当前 → 给"select one"语义。
   *
   * 调 setValue 后无需手动 update —— RAF tick 里 vrm.update(dt) 链路会自动应用表情。
   * value 范围 0-1；预设 UI 一般直接传 1（全权重）；传 0 等于关闭该表情。
   *
   * 诊断（M4）：首次调用 console.log 模型实际烘焙的 expression names，便于排查
   * 旧 VRM（VRoid 0.x）或简化模型缺某些 emotion 时 UI radio 没反应的边角。
   */
  private expressionDiagnosticLogged = false
  setExpression(name: 'neutral' | 'happy' | 'angry' | 'sad' | 'relaxed' | 'surprised', value = 1): void {
    if (!this.vrm?.expressionManager) return
    const em = this.vrm.expressionManager
    if (!this.expressionDiagnosticLogged) {
      this.expressionDiagnosticLogged = true
      const names = em.expressions.map((e) => e.expressionName)
      console.log(`[vrm] expression names baked in model: [${names.join(', ')}]`)
      // 提示缺失的标准 emotion（仅 once；UI radio 选了找不到的表情时 setValue 静默 no-op）
      const standard = ['neutral', 'happy', 'angry', 'sad', 'relaxed', 'surprised']
      const missing = standard.filter((n) => !names.includes(n))
      if (missing.length > 0) {
        console.warn(
          `[vrm] 模型未烘焙的表情: [${missing.join(', ')}] —— UI 选这些时会静默无变化（VRM 0.x 可能用 joy/sorrow/fun 等旧名）`,
        )
      }
    }
    // 先把其他 emotion 全清 0（互斥语义；blink / lookAt 不动）
    const all: Array<'neutral' | 'happy' | 'angry' | 'sad' | 'relaxed' | 'surprised'> = [
      'neutral',
      'happy',
      'angry',
      'sad',
      'relaxed',
      'surprised',
    ]
    for (const e of all) {
      if (e !== name) em.setValue(e, 0)
    }
    em.setValue(name, Math.max(0, Math.min(1, value)))
  }

  /** 通知 canvas / 窗口尺寸变化，重算渲染器 size 与相机宽高比。 */
  resize(width: number, height: number): void {
    if (!this.renderer || !this.camera || width <= 0 || height <= 0) return
    this.renderer.setSize(width, height, false)
    this.camera.aspect = width / height
    this.camera.updateProjectionMatrix()
  }

  async loadModel(url: string): Promise<void> {
    if (!this.scene || !this.renderer || !this.camera) {
      throw new Error('VRM runtime not initialized')
    }

    // #40 (ADR-025) per-model hitbox manifest hook 点：检测 <model>_hitbox.json 同名 sibling。
    // M2 不消费 manifest 内容（4 hitbox + Bone Proxy 推迟 M3+），仅留 console.info 留痕
    // 证明 hook 已 wired up，未来 M3+ 接入时只需替换内部解析逻辑。
    // 命名约定：avatar.vrm → avatar_hitbox.json（ADR-025 per-model manifest 设计原则 #2）
    void this.probeHitboxManifest(url)

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

    // lookAt 目标用一个不可见的 Object3D，每帧更新位置；three-vrm 内部按 target 朝向解算骨骼。
    // 按官方示例（pixiv/three-vrm examples/lookat.html）：target 挂在 camera 子节点下，
    // position 用相机本地坐标 —— (0,0,0) 即"看相机自己 = 看用户"，鼠标偏移直接做本地 x/y 偏移。
    // 时序好处：camera 的 matrixWorld 由 renderer 维护，子节点 worldPosition 自动正确，无需额外 update。
    if (vrm.lookAt && this.camera) {
      this.lookAtTarget = new THREE.Object3D()
      this.lookAtTarget.position.copy(this.lookAtSmoothed) // 初始 (0,0,0) = 直视用户
      this.camera.add(this.lookAtTarget)
      vrm.lookAt.target = this.lookAtTarget
      // 诊断：lookAt applier 类型（Bone / Expression），缺失时静默降级（无视线跟随但不崩）
      const applierName = (vrm.lookAt as { applier?: { constructor: { name: string } } }).applier
        ?.constructor.name
      console.log(`[vrm] lookAt enabled, applier=${applierName ?? 'unknown'}`)
    } else {
      console.warn(
        '[vrm] this model has no lookAt rig — 视线跟随不可用（需要 VRM 烘焙 firstPerson + lookAt）',
      )
    }

    // 第一次眨眼倒计时：等 1s 缓冲（loadModel 后立刻眨眼会显得突兀），其余周期走随机区间
    this.blinkCountdown = 1

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
        // 顺序很重要：所有 humanoid bone / expression / lookAt 修改必须在 vrm.update(dt) 之前。
        // vrm.update 内部按"应用 humanoid → expressionManager.applyWeights → lookAt → springBone"
        // 链路结算，提前改才会被正确合并；放在 update 之后改 bone 会被下一帧 spring bone 物理覆盖
        // 或被错误地"先解算后修改"（参考 pixiv/three-vrm 多个 spring/lookAt 抖动 issue）。
        this.applyBreathing(dt)
        this.applyBlink(dt)
        this.applyLookAt(dt)
        this.vrm.update(dt)
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

  /**
   * 眨眼：随机 4-8s 触发一次，0-1-0 三角包络共 150ms。
   * 用 expressionManager.setValue('blink', w)，每帧 reset 后写入（防止 override 残留导致眼睛闭着不睁开）。
   * 若 VRM 没烘焙 blink 表情则静默跳过（部分简化模型不带 blink，不应崩溃）。
   */
  private applyBlink(dt: number): void {
    const em = this.vrm?.expressionManager
    if (!em) return

    // 每帧强制清零，避免上一帧残留 + 防止其它逻辑（未来 lipSync / 情绪）累加污染
    em.setValue('blink', 0)

    if (this.blinkProgress < 0) {
      // 倒计时阶段：等下一次眨眼触发
      this.blinkCountdown -= dt
      if (this.blinkCountdown <= 0) {
        this.blinkProgress = 0
      }
      return
    }

    // 眨眼进行中：用三角包络（0→1→0），峰值在 BLINK_DURATION_S/2
    this.blinkProgress += dt
    const t = this.blinkProgress / VRMRuntime.BLINK_DURATION_S
    if (t >= 1) {
      // 本次眨眼结束：随机下一次间隔，进入倒计时
      this.blinkProgress = -1
      this.blinkCountdown =
        VRMRuntime.BLINK_INTERVAL_MIN_S +
        Math.random() * (VRMRuntime.BLINK_INTERVAL_MAX_S - VRMRuntime.BLINK_INTERVAL_MIN_S)
      return
    }
    // 三角函数包络：sin(πt) 在 [0,1] 上是 0→1→0，比线性三角更柔和
    em.setValue('blink', Math.sin(Math.PI * t))
  }

  /**
   * 视线跟随：鼠标 NDC 直接作为 lookAt target 的**相机本地坐标**。
   * 帧率无关的指数平滑避免目标突跳引发头部抖动，且在低 fps 下跟随时间稳定。
   * - target 是 camera 子节点（loadModel 里 setup），position=(0,0,0) 即"看用户"
   * - cursorNdc=null（鼠标离开 canvas）→ desired 归零（视线归正，直视用户）
   * - VRM 没 lookAt 组件 → 静默跳过
   *
   * 选 0.6 倍 NDC 作为偏移系数：鼠标到 canvas 角时偏移 0.6 单位（相机本地），
   * 配合 fov 30° / 半身距离 1.5，约对应视线偏角 ±22°，自然但不夸张。
   */
  private applyLookAt(dt: number): void {
    if (!this.vrm?.lookAt || !this.lookAtTarget) return

    if (this.cursorNdc) {
      // 相机本地系：+x 右、+y 上；NDC y 已翻转过（PetCanvas 里），这里直接用
      // z 留 0：目标恰好在相机所在平面上 ≈ 看用户脸的位置（z 略负会让目标在相机前方 = 视线穿过用户）
      this.lookAtDesired.set(this.cursorNdc.x * 0.6, this.cursorNdc.y * 0.6, 0)
    } else {
      // cursorNdc=null：归零 = 直视相机/用户，最自然的"待机"视线
      this.lookAtDesired.set(0, 0, 0)
    }

    // 帧率无关指数平滑：α = 1 - exp(-dt / τ)，等价于"每经过 τ 秒衰减 63%"。
    // 第一帧 dt=0 → α=0 → 不动；正常 60fps 下 dt≈0.0167 → α≈0.154，与原 0.15 几乎一致。
    const alpha = 1 - Math.exp(-dt / VRMRuntime.LOOK_AT_TAU_S)
    this.lookAtSmoothed.lerp(this.lookAtDesired, alpha)
    this.lookAtTarget.position.copy(this.lookAtSmoothed)
  }

  /**
   * 上报光标在 canvas 内的 NDC 坐标（x/y ∈ [-1, 1]，y 已翻转）。
   * 传 null 表示鼠标离开 → 视线回中。供 PetCanvas pointermove / pointerleave 调用。
   */
  setCursorNdc(ndc: { x: number; y: number } | null): void {
    this.cursorNdc = ndc
  }

  /**
   * 把当前帧渲染并以 PNG data URL 返回（#26 VRM 头像导出）。
   *
   * @param size 目标 PNG 边长（正方形）。可选；不传 = 用 canvas 当前 buffer 尺寸。
   *
   * 实现要点：
   * - **同步 render → toDataURL**：WebView2/Chromium 在同步链内即可拿到 buffer，无需
   *   preserveDrawingBuffer。任何 await / setTimeout 插入这两步之间都可能拿到空白。
   * - **DPR 处理（H1 修复）**：init 时 `setPixelRatio(devicePixelRatio)`，setSize(N, N) 在
   *   1.5×/2× DPR 下实际产生 N*dpr × N*dpr 的 PNG。截图前临时 setPixelRatio(1) 保证落盘
   *   是精确 size×size；截图后恢复原 DPR 让预览渲染保持锐利。
   * - **视线 / 表情归零**：cursorNdc 可能残留 + applyLookAt 是指数平滑，截图等不起几帧；
   *   直接清 lookAtSmoothed + blink expression，给静态歇息相。其它 emotion（happy/sad
   *   等用户主动设的）不动 —— 仅清 blink 的"无意识动作"。
   * - **vrm.update(0)**：让 expression / lookAt 链路 settle 到归零状态，dt=0 不推进 breath。
   *
   * 返 'data:image/png;base64,...' 形式字符串。
   */
  captureSnapshot(size?: number): string {
    if (!this.renderer || !this.scene || !this.camera || !this.vrm) {
      throw new Error('VRM runtime not ready (init + loadModel must finish first)')
    }

    // H1 修复：临时降 pixelRatio 到 1，让 setSize(size,size) 输出真实 size×size buffer
    const originalDpr = this.renderer.getPixelRatio()
    const dprChanged = size !== undefined && originalDpr !== 1
    // 记下原 buffer 尺寸（恢复用）
    const originalSize = new THREE.Vector2()
    this.renderer.getSize(originalSize)
    const sizeChanged = size !== undefined && (originalSize.x !== size || originalSize.y !== size)

    try {
      if (dprChanged) {
        this.renderer.setPixelRatio(1)
      }
      if (sizeChanged && size !== undefined) {
        this.renderer.setSize(size, size, false)
        this.camera.aspect = 1
        this.camera.updateProjectionMatrix()
      }

      // 1) 视线归正（applyLookAt 指数平滑需几帧才回中，截图等不起）
      this.cursorNdc = null
      this.lookAtSmoothed.set(0, 0, 0)
      if (this.lookAtTarget) this.lookAtTarget.position.set(0, 0, 0)
      // 2) 清 blink 残留（其他 emotion 不动 —— 用户在 UI 主动选的表情保留）
      const em = this.vrm.expressionManager
      if (em) em.setValue('blink', 0)
      // 3) update(0) 不推进 breath / blink countdown，仅让 lookAt / expression 链路 settle
      this.vrm.update(0)
      // 4) 同步 render → toDataURL，buffer 在同链内可读
      this.renderer.render(this.scene, this.camera)
      return this.renderer.domElement.toDataURL('image/png')
    } finally {
      // 恢复 dpr + size，确保 live 预览继续锐利渲染（finally 保证异常路径也复原）
      if (dprChanged) {
        this.renderer.setPixelRatio(originalDpr)
      }
      if (sizeChanged) {
        this.renderer.setSize(originalSize.x, originalSize.y, false)
        this.camera.aspect = originalSize.x / originalSize.y
        this.camera.updateProjectionMatrix()
      }
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

  /** 暂停 RAF 循环（不卸载资源）；resumeLoop 恢复。idempotent（已暂停时 no-op）。
   *  用例：#26 头像导出器在 settings tab 切走 / 窗口隐藏时，省 GPU/CPU 但保留加载好的 VRM。 */
  pauseLoop(): void {
    if (this.rafId !== null) {
      cancelAnimationFrame(this.rafId)
      this.rafId = null
      // 清掉 lastFrameMs，恢复后 dt 从 0 开始（避免大 dt 让 breath/blink 跳跃）
      this.lastFrameMs = null
    }
  }

  /** 恢复 RAF 循环（仅当已 init 且 VRM 已加载时有效）；idempotent。 */
  resumeLoop(): void {
    if (this.rafId !== null) return // 已在跑
    if (!this.vrm || !this.renderer) return // 还没就绪，loadModel 完成时会自然 startLoop
    this.startLoop()
  }

  /**
   * 播放命名动作。M2 W3 实现 'nod' / 'glance_up' / 'glance_down'（#29 + 2026-05-25 spec），其他 #23 接入 reaction_table 时填。
   * vrm 未 ready 时静默 no-op（reminder:fired 可能在 VRM 加载完成前到达）。
   */
  async playAction(actionId: PetActionId): Promise<void> {
    if (!this.vrm) {
      // 静默 no-op：onboarding 期 / VRM 加载失败时 reminder:fired 仍会触发；
      // 此处不报错不弹 toast（spec §8.2 + R8）。
      return
    }
    if (actionId === 'nod' || actionId === 'glance_up') {
      await this.playGlance(1)
      return
    }
    if (actionId === 'glance_down') {
      await this.playGlance(-1)
      return
    }
    // #23 placeholder：其他 actionId 走 dev 警告 + no-op
    if (import.meta.env.DEV) {
      console.warn('[vrm] playAction not implemented:', actionId)
    }
  }

  /**
   * 短促 head glance 动效：head bone X 轴 ±15° / 360ms RAF 插值（不引动画 clip）。
   *
   * @param sign +1 = 抬头看上方（glance_up / nod 兼容路径），-1 = 低头看下方（glance_down，
   *             用于 reminder overlay 在 pet 下方时；spec 2026-05-25-pet-reminder-card-stack §5）。
   *
   * 不打断 wander tween，不持久化（瞬时动效）。
   * head bone 的 rotation 不被 applyBreathing/applyBlink/applyLookAt 任一改写
   * （那三个分别动 chest / expression / lookAtTarget），所以直接写 rotation.x 安全。
   * _glanceInProgress 标志保证并发 fired 不会 interleaved RAF 导致 baseX 漂移。
   */
  private async playGlance(sign: 1 | -1): Promise<void> {
    if (!this.vrm || this._glanceInProgress) return
    this._glanceInProgress = true
    const humanoid = this.vrm.humanoid
    if (!humanoid) return
    const headNode = humanoid.getNormalizedBoneNode('head')
    if (!headNode) return

    const baseX = headNode.rotation.x
    const peakDelta = (sign * 15 * Math.PI) / 180 // +15° 抬头 / -15° 低头
    const duration = 360
    const start = performance.now()

    return new Promise<void>((resolve) => {
      const tick = (t: number) => {
        const elapsed = t - start
        if (elapsed >= duration) {
          headNode.rotation.x = baseX
          this._glanceInProgress = false
          resolve()
          return
        }
        const p = elapsed / duration // 0..1
        // 三角包络：0 → 1 → 0
        const tri = p < 0.5 ? p * 2 : (1 - p) * 2
        headNode.rotation.x = baseX + peakDelta * tri
        requestAnimationFrame(tick)
      }
      requestAnimationFrame(tick)
    })
  }

  /**
   * #40 (ADR-025) per-model hitbox manifest 检测 hook 点。
   *
   * 约定（ADR-025 per-model manifest 设计原则）：
   * - 命名：`<model_basename>_hitbox.json`（与模型同目录，如 `/avatar/avatar.vrm` → `/avatar/avatar_hitbox.json`）
   * - manifest 是 **optional capability**：用户可自主切换 / 上传模型；缺失 → 自动 AABB 单 body 降级
   * - M2 期：本方法**不消费** manifest 内容（仅 HEAD probe + console.info 留痕）
   * - M3+ 期：本方法替换为真实 parse + attach Bone Proxy 4 hitbox 到 humanoid bone
   *
   * 失败降级：HEAD 请求失败 / 网络异常 / model URL 非 .vrm → 静默走默认 AABB 路径（不阻塞 loadModel）。
   * fire-and-forget：不让 manifest probe 阻塞 VRM 加载（probe 耗时 < manifest 解析 ≪ VRM 加载）。
   */
  private async probeHitboxManifest(modelUrl: string): Promise<void> {
    if (!modelUrl.toLowerCase().endsWith('.vrm')) {
      console.info('[vrm] hitbox manifest hook skipped: model url is not .vrm')
      return
    }
    const manifestUrl = modelUrl.replace(/\.vrm$/i, '_hitbox.json')
    try {
      // HEAD 不读 body；Vite static serve / Tauri asset protocol 都接 HEAD。
      // 404 / network err → 走 AABB 降级（M2 默认路径）。
      const res = await fetch(manifestUrl, { method: 'HEAD' })
      if (res.ok) {
        console.info(
          `[vrm] hitbox manifest hook checked: ${manifestUrl} found → AABB fallback (parse path lands in M3+)`,
        )
      } else {
        console.info(
          `[vrm] hitbox manifest hook checked: ${manifestUrl} missing (HTTP ${res.status}) → AABB fallback`,
        )
      }
    } catch (e) {
      console.info(
        `[vrm] hitbox manifest hook checked: ${manifestUrl} probe failed (${(e as Error).message}) → AABB fallback`,
      )
    }
  }

  destroy(): void {
    if (this.rafId !== null) {
      cancelAnimationFrame(this.rafId)
      this.rafId = null
    }
    if (this.lookAtTarget && this.camera) {
      this.camera.remove(this.lookAtTarget)
      this.lookAtTarget = null
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
