package {{PACKAGE}}

import android.os.Bundle
import androidx.core.view.WindowCompat

/**
 * 覆盖 Tauri 默认模板的 MainActivity。
 *
 * 官方模板调用 enableEdgeToEdge()，会让 WebView 延伸到状态栏下方；
 * 而 Android WebView 对 env(safe-area-inset-*) 的支持不可靠（常返回 0），
 * 导致页面顶部内容被状态栏遮挡。
 *
 * 这里改为让系统为状态栏/导航栏预留空间（decorFitsSystemWindows = true），
 * WebView 从状态栏下方开始绘制，内容不再被遮挡。
 * 不依赖任何 CSS 安全区变量，因此在所有 Android 版本上都可靠。
 */
class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    WindowCompat.setDecorFitsSystemWindows(window, true)
    super.onCreate(savedInstanceState)
  }
}
