package {{PACKAGE}}

import android.graphics.Color
import android.os.Build
import android.os.Bundle
import android.view.View
import android.view.WindowInsets
import android.view.WindowInsetsController

/**
 * 覆盖 Tauri 官方模板的 MainActivity（模板第一行就是 enableEdgeToEdge()）。
 *
 * 关键事实：Android 15（API 35）起，targetSdk >= 35 的应用会被系统强制
 * edge-to-edge，且没有任何开关可以关闭（windowOptOutEdgeToEdgeEnforcement
 * 从 API 36 起也被忽略）。所以「删掉 enableEdgeToEdge()」并不能避免内容被
 * 状态栏压住 —— 这是前几次修复无效的真正原因。
 *
 * 现在改为主动处理窗口 inset：
 *   1. 按状态栏高度给内容视图加 top padding，页面真正从状态栏下方开始绘制；
 *   2. padding 那一条用品牌蓝填充（WebView 会盖住其余部分），
 *      视觉上和蓝色顶栏连成一体，不会出现白条；
 *   3. 状态栏图标固定白色。
 *
 * 只用平台 API，不引入 androidx 依赖，避免模板依赖差异导致编译失败。
 */
@Suppress("DEPRECATION")
class MainActivity : TauriActivity() {

  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)

    val brandBlue = Color.rgb(0x25, 0x63, 0xEB)

    // 状态栏图标用白色（配合品牌蓝底）
    if (Build.VERSION.SDK_INT >= 30) {
      window.insetsController?.setSystemBarsAppearance(
        0,
        WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS,
      )
    } else {
      window.decorView.systemUiVisibility =
        window.decorView.systemUiVisibility and View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR.inv()
    }

    // API < 35 没有被强制 edge-to-edge，让窗口自己适配系统栏即可
    if (Build.VERSION.SDK_INT < 35) {
      window.statusBarColor = brandBlue
      if (Build.VERSION.SDK_INT >= 30) {
        window.setDecorFitsSystemWindows(true)
      }
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
    // 布局完成后再请求一次，防止首帧拿到 0 高度的 inset
    content.post { content.requestApplyInsets() }
  }
}
