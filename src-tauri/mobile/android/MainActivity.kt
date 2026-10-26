package {{PACKAGE}}

import android.graphics.Color
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.View
import android.view.WindowInsets
import android.view.WindowInsetsController

/**
 * 覆盖 Tauri 官方模板的 MainActivity（模板里第一行就是 enableEdgeToEdge()）。
 *
 * 为什么必须覆盖：Android 15（API 35）起，targetSdk >= 35 的应用会被系统强制
 * edge-to-edge，且 windowOptOutEdgeToEdgeEnforcement 从 API 36 起也被忽略。
 * 也就是说「删掉 enableEdgeToEdge()」并不能阻止内容被状态栏压住，
 * 必须自己按窗口 inset 给内容让出空间。
 *
 * 做三件事：
 *   1. 按状态栏高度给内容视图加 top padding，页面从状态栏下方开始绘制；
 *   2. padding 那一条用品牌蓝填充，与 WebView 顶栏同色，视觉上连成一体；
 *   3. 状态栏图标固定白色。
 *
 * 稳健性：所有窗口相关调用都包在 try/catch 里。这类 API 在不同 ROM 和
 * API 级别上行为差异很大，一旦在 onCreate 抛异常就是整个 Activity 起不来
 * （直接闪退），代价远大于顶部多一条留白。
 */
class MainActivity : TauriActivity() {

  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)

    try {
      applyBrandWindow()
    } catch (t: Throwable) {
      // 宁可顶部留白不对，也不能让应用起不来
      Log.w(TAG, "窗口适配失败，回退系统默认行为", t)
    }
  }

  @Suppress("DEPRECATION")
  private fun applyBrandWindow() {
    val brandBlue = Color.rgb(0x25, 0x63, 0xEB)

    // 必须先取 DecorView。
    // window.insetsController 内部会直接解引用 mDecor，
    // DecorView 尚未创建时调用会抛 NullPointerException —— v1.0.8 闪退正是这个原因。
    // 访问 decorView 会强制 installDecor()，之后这些调用才是安全的。
    val decor = window.decorView

    // 状态栏图标用白色，配合品牌蓝底（两种写法都清掉 LIGHT_STATUS_BAR 标志）
    if (Build.VERSION.SDK_INT >= 30) {
      decor.windowInsetsController?.setSystemBarsAppearance(
        0,
        WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS,
      )
    } else {
      decor.systemUiVisibility =
        decor.systemUiVisibility and View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR.inv()
    }

    // API < 35 没有被强制 edge-to-edge，直接给状态栏上色即可
    if (Build.VERSION.SDK_INT < 35) {
      window.statusBarColor = brandBlue
    }

    val content: View = findViewById(android.R.id.content)
    // 只有 padding 区域会露出这个底色（WebView 铺满其余区域）
    content.setBackgroundColor(brandBlue)
    content.setOnApplyWindowInsetsListener { view, insets ->
      val top = if (Build.VERSION.SDK_INT >= 30) {
        val types = WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout()
        insets.getInsets(types).top
      } else {
        insets.systemWindowInsetTop
      }
      if (view.paddingTop != top) {
        view.setPadding(0, top, 0, 0)
      }
      insets
    }
    content.requestApplyInsets()
  }

  private companion object {
    const val TAG = "MainActivity"
  }
}
