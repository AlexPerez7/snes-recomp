package io.github.alekaydevs.snesport;

import android.content.Context;
import android.content.res.ColorStateList;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.StateListDrawable;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import android.widget.CheckBox;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.TextView;

/** El aspecto del lanzador, el mismo que el de Windows y Linux: piedra verde con damero, marco dorado, Trifuerza y botones de placa. */
final class ZeldaUi {
    static final int OUTLINE = 0xFF2A1A06, GOLD_HI = 0xFFFFF0A0, GOLD = 0xFFF8D860, GOLD_LO = 0xFFC08820;
    static final int TEXT = 0xFFC8D8B8, TEXT_HI = 0xFFFFFCE8;

    private ZeldaUi() {}

    static int dp(Context c, float v) {
        return Math.round(v * c.getResources().getDisplayMetrics().density);
    }

    /** El fondo con el marco: degradado verde oscuro, damero tenue, tres filetes dorados y un rombo en cada esquina. */
    static final class Backdrop extends FrameLayout {
        private final Paint p = new Paint(Paint.ANTI_ALIAS_FLAG);
        private int insL, insT, insR, insB;

        Backdrop(Context c) {
            super(c);
            setWillNotDraw(false);
            // El marco y los rombos quedan dentro de las barras del sistema; el fondo llega hasta el borde.
            setOnApplyWindowInsetsListener((v, ins) -> {
                insL = ins.getSystemWindowInsetLeft(); insT = ins.getSystemWindowInsetTop();
                insR = ins.getSystemWindowInsetRight(); insB = ins.getSystemWindowInsetBottom();
                setPadding(insL, insT, insR, insB);
                invalidate();
                return ins;
            });
        }

        @Override
        protected void onDraw(Canvas cv) {
            int w = getWidth(), h = getHeight();
            Context c = getContext();
            p.setStyle(Paint.Style.FILL);
            p.setShader(new android.graphics.LinearGradient(0, 0, 0, h, 0xFF0C1C14, 0xFF1A3828, android.graphics.Shader.TileMode.CLAMP));
            cv.drawRect(0, 0, w, h, p);
            p.setShader(null);
            p.setColor(0x182E5A40 | 0x10000000);
            int s = dp(c, 24);
            for (int y = 0; y * s < h; y++)
                for (int x = 0; x * s < w; x++)
                    if (((x + y) & 1) == 0) cv.drawRect(x * s, y * s, x * s + s, y * s + s, p);
            int[] col = {OUTLINE, GOLD, GOLD_LO};
            p.setStyle(Paint.Style.STROKE);
            p.setStrokeWidth(dp(c, 2));
            for (int i = 0; i < 3; i++) {
                p.setColor(col[i]);
                float m = dp(c, 10 + i * 3);
                cv.drawRect(insL + m, insT + m, w - insR - m, h - insB - m, p);
            }
            p.setStyle(Paint.Style.FILL);
            float r = dp(c, 9), o = dp(c, 24);
            for (int i = 0; i < 4; i++) {
                float cx = (i & 1) != 0 ? w - insR - o : insL + o, cy = (i & 2) != 0 ? h - insB - o : insT + o;
                Path d = new Path();
                d.moveTo(cx, cy - r); d.lineTo(cx + r, cy); d.lineTo(cx, cy + r); d.lineTo(cx - r, cy); d.close();
                p.setColor(GOLD_LO);
                cv.drawPath(d, p);
                p.setColor(GOLD);
                Path top = new Path();
                top.moveTo(cx, cy - r); top.lineTo(cx + r, cy); top.lineTo(cx, cy); top.lineTo(cx - r, cy); top.close();
                cv.drawPath(top, p);
            }
        }
    }

    /** Tres triángulos dorados con bisel: la Trifuerza. */
    static final class Triforce extends View {
        private final Paint p = new Paint(Paint.ANTI_ALIAS_FLAG);

        Triforce(Context c) {
            super(c);
        }

        @Override
        protected void onMeasure(int wSpec, int hSpec) {
            setMeasuredDimension(dp(getContext(), 96), dp(getContext(), 84));
        }

        private void piece(Canvas cv, float cx, float top, float w, float h) {
            Path t = new Path();
            t.moveTo(cx, top); t.lineTo(cx + w / 2, top + h); t.lineTo(cx - w / 2, top + h); t.close();
            p.setStyle(Paint.Style.FILL);
            p.setColor(GOLD);
            cv.drawPath(t, p);
            Path shade = new Path(); // el lado de la sombra: derecha y abajo
            shade.moveTo(cx, top); shade.lineTo(cx + w / 2, top + h); shade.lineTo(cx + w / 2 - w * 0.12f, top + h); shade.lineTo(cx, top + h * 0.2f); shade.close();
            p.setColor(GOLD_LO);
            cv.drawPath(shade, p);
            p.setStyle(Paint.Style.STROKE);
            p.setStrokeWidth(dp(getContext(), 1.5f));
            p.setColor(OUTLINE);
            cv.drawPath(t, p);
        }

        @Override
        protected void onDraw(Canvas cv) {
            float w = getWidth() / 2f, tw = dp(getContext(), 48), th = dp(getContext(), 42);
            piece(cv, w, 0, tw, th);
            piece(cv, w - tw / 2, th, tw, th);
            piece(cv, w + tw / 2, th, tw, th);
        }
    }

    /** El botón de idioma: globo y el código del idioma actual en una placa dorada, arriba a la derecha. */
    static final class LangButton extends View {
        private final Paint p = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final String code;

        LangButton(Context c, String code) {
            super(c);
            this.code = code.toUpperCase();
            setClickable(true);
            setContentDescription(c.getString(R.string.language));
        }

        @Override
        protected void onMeasure(int wSpec, int hSpec) {
            setMeasuredDimension(dp(getContext(), 72), dp(getContext(), 40));
        }

        @Override
        protected void onDraw(Canvas cv) {
            Context c = getContext();
            float w = getWidth(), h = getHeight(), r = h / 2 - dp(c, 2);
            p.setStyle(Paint.Style.FILL);
            p.setColor(isPressed() ? 0xFF2E6A48 : 0xFF16382A);
            cv.drawRoundRect(dp(c, 2), dp(c, 2), w - dp(c, 2), h - dp(c, 2), r, r, p);
            p.setStyle(Paint.Style.STROKE);
            p.setStrokeWidth(dp(c, 2));
            p.setColor(GOLD);
            cv.drawRoundRect(dp(c, 2), dp(c, 2), w - dp(c, 2), h - dp(c, 2), r, r, p);
            float gx = dp(c, 22), gy = h / 2, gr = dp(c, 8);
            p.setStrokeWidth(dp(c, 1.5f));
            p.setColor(TEXT_HI);
            cv.drawCircle(gx, gy, gr, p);
            cv.drawLine(gx - gr, gy, gx + gr, gy, p);
            cv.drawLine(gx, gy - gr, gx, gy + gr, p);
            cv.drawOval(gx - gr * 0.5f, gy - gr, gx + gr * 0.5f, gy + gr, p);
            p.setStyle(Paint.Style.FILL);
            p.setTypeface(Typeface.create(Typeface.DEFAULT, Typeface.BOLD));
            p.setTextSize(dp(c, 15));
            p.setColor(GOLD_HI);
            cv.drawText(code, dp(c, 36), gy + dp(c, 5.5f), p);
        }
    }

    private static GradientDrawable plate(Context c, int fill, int stroke) {
        GradientDrawable g = new GradientDrawable();
        g.setColor(fill);
        g.setCornerRadius(dp(c, 30));
        g.setStroke(dp(c, 2), stroke);
        return g;
    }

    /** Un botón de placa: el principal (grande, verde claro) o uno secundario. */
    static Button button(Context c, CharSequence label, boolean primary) {
        Button b = new Button(c);
        b.setText(label);
        b.setAllCaps(true);
        b.setStateListAnimator(null);
        b.setTypeface(Typeface.create(Typeface.DEFAULT, Typeface.BOLD));
        b.setTextSize(primary ? 22 : 15);
        b.setTextColor(primary ? GOLD_HI : 0xFFE8E0B0);
        b.setLetterSpacing(0.06f);
        StateListDrawable bg = new StateListDrawable();
        bg.addState(new int[] {android.R.attr.state_pressed}, plate(c, 0xFF3B8058, GOLD_HI));
        bg.addState(new int[] {}, plate(c, primary ? 0xFF2B6B47 : 0xFF16382A, primary ? GOLD_HI : GOLD));
        b.setBackground(bg);
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(c, primary ? 64 : 50));
        lp.topMargin = dp(c, primary ? 20 : 12);
        b.setLayoutParams(lp);
        return b;
    }

    static TextView text(Context c, CharSequence s, float sp, int color, boolean bold) {
        TextView t = new TextView(c);
        t.setText(s);
        t.setTextSize(sp);
        t.setTextColor(color);
        t.setGravity(Gravity.CENTER);
        if (bold) t.setTypeface(Typeface.create(Typeface.DEFAULT, Typeface.BOLD));
        return t;
    }

    static CheckBox check(Context c, CharSequence label, boolean on) {
        CheckBox b = new CheckBox(c);
        b.setText(label);
        b.setTextColor(TEXT);
        b.setTextSize(14);
        b.setChecked(on);
        b.setButtonTintList(ColorStateList.valueOf(GOLD));
        return b;
    }
}
