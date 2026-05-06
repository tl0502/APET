import { onBeforeUnmount, onMounted, ref, type Ref } from 'vue'
import { VRMRuntime } from '@/services/vrm'

interface MemoryInfo {
  usedJSHeapSize: number
}

/**
 * VRM 模型挂载 composable：负责 init + loadModel + dispose 生命周期。
 * 性能日志（start_ms / heap_mb）仅 dev 期可见；prod build 时 vite esbuild.drop 会清掉 console。
 */
export function useVRMModel(canvasRef: Ref<HTMLCanvasElement | null>, modelUrl: string) {
  const runtime = new VRMRuntime()
  const isLoaded = ref(false)
  const errorMessage = ref<string | null>(null)

  onMounted(async () => {
    if (!canvasRef.value) return

    performance.mark('vrm-start')
    try {
      runtime.init(canvasRef.value)
      await runtime.loadModel(modelUrl)
      performance.mark('vrm-loaded')

      const measure = performance.measure('vrm-load', 'vrm-start', 'vrm-loaded')
      console.log(`[vrm] start_ms=${measure.duration.toFixed(0)}`)

      const memory = (performance as unknown as { memory?: MemoryInfo }).memory
      if (memory) {
        console.log(`[vrm] heap_mb=${(memory.usedJSHeapSize / 1024 / 1024).toFixed(1)}`)
      }

      isLoaded.value = true
    } catch (err) {
      errorMessage.value = err instanceof Error ? err.message : String(err)
      console.error('[vrm] load failed:', err)
    }
  })

  onBeforeUnmount(() => {
    runtime.destroy()
  })

  return { isLoaded, errorMessage, runtime }
}
