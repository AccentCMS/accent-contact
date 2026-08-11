/**
 * Reading time island component (accent-contact plugin)
 *
 * Estimates reading time based on word count (~200 wpm).
 * Demonstrates a plugin-provided island with visible hydration.
 */
if (window.AccentIslands) {
window.AccentIslands.register("accent-contact:reading-time", function (el, props) {
    var wpm = props.wpm || 200;
    var target = props.target || "article";
    var article = document.querySelector(target);
    if (!article) return;

    var text = article.textContent || "";
    var words = text.trim().split(/\s+/).filter(function (w) { return w.length > 0; }).length;
    var minutes = Math.max(1, Math.ceil(words / wpm));

    el.textContent = minutes + " min read";
});
}
