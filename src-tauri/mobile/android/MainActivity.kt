package {{PACKAGE}}

import android.graphics.Color
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.View
import android.view.WindowInsetsController

/**
 * 覆盖 Tauri 官方模板的 MainActivity（模板第一行就是 enableEdgeToEdge()）。
 *
 * 只做一件事：把状态栏恢复成系统默认外观（透明底 + 深色图标），
 * 让蓝色标题栏从状态栏下方开始，不再和状态栏连成一整条很高的蓝带。
 *
 * 关键点：
 *   1. 绝对不能给内容视图补 padding！系统本身就会让位，补了反而多出空白。
 *   2. 不能给状态栏上品牌蓝，否则状态栏 + 标题栏连成一片，看起来非常高。
 *
 * 踩过的坑（v1.0.8 闪退的直接原因）：
 *   window.insetsController 在 onCreate 阶段会 NPE —— PhoneWindow.getInsetsController()
 *   内部直接解引用 mDecor，而 DecorView 此时还没创建，Kotlin 的 `?.` 挡不住。
 *   必须先访问一次 window.decorView 触发 installDecor()。
 */
class MainActivity : TauriActivity() {

  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)

    try {
      applyStatusBar()
    } catch (t: Throwable) {
      Log.w(TAG, "状态栏设置失败，回退系统默认行为", t)
    }
  }

  @Suppress("DEPRECATION")
  private fun applyStatusBar() {
    // 必须先取 DecorView：强制 installDecor()，之后 window.insetsController 才不是 null
    val decor: View = window.decorView

    if (Build.VERSION.SDK_INT >= 35) {
      // API 35+ 强制 edge-to-edge，状态栏本身就是透明的，无需处理
      return
    }

    // 状态栏透明：底色由窗口背景决定（白色），和页面顶部自然衔接
    window.statusBarColor = Color.TRANSPARENT

    // 浅色背景 → 用深色图标，保证时间和电量看得清
    if (Build.VERSION.SDK_INT >= 30) {
      decor.windowInsetsController?.setSystemBarsAppearance(
        WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS,
        WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS,
      )
    } else {
      decor.systemUiVisibility =
        decor.systemUiVisibility or View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR
    }
  }

  private companion object {
    const val TAG = "MainActivity"
  }
}