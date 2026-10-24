package {{PACKAGE}}

import android.os.Bundle

/**
 * 覆盖 Tauri 默认模板的 MainActivity。
 *
 * 官方模板调用 enableEdgeToEdge()，让 WebView 延伸到状态栏下方；
 * 而 Android WebView 的 env(safe-area-inset-top) 实际返回 0，
 * 纯 CSS 方案无法避让，导致页面顶部内容被状态栏遮挡。
 *
 * 这里不再启用 edge-to-edge：Activity 保持默认的 decorFitsSystemWindows，
 * 系统会为状态栏和导航栏预留空间，WebView 从状态栏下方开始绘制。
 *
 * 不引入 androidx 额外依赖，避免不同 Tauri 版本模板依赖差异导致编译失败。
 */
class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
  }
}
