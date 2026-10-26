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
 * 三种设备情况，必须区别对待（否则要么被状态栏遮挡，要么多出一条空白）：
 *
 *   A. Android < 15（API < 35）
 *      删掉模板的 enableEdgeToEdge() 之后，窗口恢复默认的 decorFitsSystemWindows=true，
 *      系统自己会给状态栏让位，WebView 从状态栏下方开始绘制 —— 不需要补 padding。
 *      本机实测（Android 13 / API 33）：去掉 enableEdgeToEdge 后布局完全正常。
 *      这里只把状态栏染成品牌蓝，否则会露出 Material3 默认的紫色。
 *
 *   B. Android 15+（API >= 35）且 targetSdk >= 35
 *      系统强制 edge-to-edge，无法关闭（windowOptOutEdgeToEdgeEnforcement 从 API 36 起也失效）。
 *      此时才需要自己读窗口 inset 并给内容视图补 top padding。
 *
 *   C. 任何版本上的异常
 *      窗口 API 在各家 ROM 上差异很大，onCreate 里抛异常就是整个 Activity 起不来。
 *      所以全部包在 try/catch 里：宁可顶部留白不对，也不能让应用打不开。
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
      applyBrandWindow()
    } catch (t: Throwable) {
      Log.w(TAG, "窗口适配失败，回退系统默认行为", t)
    }
  }

  private fun applyBrandWindow() {
    val brandBlue = Color.rgb(0x25, 0x63, 0xEB)

    // 必须先取 DecorView：强制 installDecor()，之后 window.insetsController 才不是 null
    val decor: View = window.decorView

    // 状态栏图标用白色（品牌蓝底 + 深色图标会看不清）
    if (Build.VERSION.SDK_INT >= 30) {
      decor.windowInsetsController?.setSystemBarsAppearance(
        0,
        WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS,
      )
    } else {
      setLegacyLightStatusBar(decor, false)
    }

    if (Build.VERSION.SDK_INT >= 35) {
      // 强制 edge-to-edge：自己补 padding
      padForStatusBar(brandBlue)
    } else {
      // 系统已经让位，只需要把状态栏染色
      setLegacyStatusBarColor(brandBlue)
    }
  }

  /** 仅 API >= 35 使用：给内容视图补状态栏高度的 padding，并用品牌蓝填充该区域。 */
  private fun padForStatusBar(brandBlue: Int) {
    val content: View = findViewById(android.R.id.content) ?: return
    content.setBackgroundColor(brandBlue)
    content.setOnApplyWindowInsetsListener { view, insets ->
      val types = WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout()
      val top = insets.getInsets(types).top
      if (view.paddingTop != top) {
        view.setPadding(0, top, 0, 0)
      }
      insets
    }
    content.requestApplyInsets()
  }

  @Suppress("DEPRECATION")
  private fun setLegacyStatusBarColor(color: Int) {
    window.statusBarColor = color
  }

  @Suppress("DEPRECATION")
  private fun setLegacyLightStatusBar(view: View, light: Boolean) {
    view.systemUiVisibility = if (light) {
      view.systemUiVisibility or View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR
    } else {
      view.systemUiVisibility and View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR.inv()
    }
  }

  private companion object {
    const val TAG = "MainActivity"
  }
}
