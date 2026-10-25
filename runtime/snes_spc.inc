/* snes_spc.inc - the sound side of the console: SPC700 CPU with its 64 KiB RAM, boot ROM, ports and timers,
 * and the DSP (8 BRR voices, envelopes, noise, pitch modulation, echo). Included by snes_core.inc.
 *
 * The main CPU talks to it through four ports: it writes spc_in[] (what the SPC700 reads at $F4-$F7) and reads
 * spc_out[] (what the SPC700 writes there). The clock is driven by the main clock of the runtime: spc_advance()
 * gives the SPC700 the cycles that correspond to a number of ticks of the frame (17039 SPC700 cycles per frame).
 * Every 32 SPC700 cycles the DSP makes one stereo sample (32 kHz), which goes to audio_hook if there is one.
 *
 * The voice interpolation uses a bell-shaped kernel computed at start, close to the Gaussian one of the real
 * DSP, not the exact table. */

static uint8_t aram[0x10000];
static uint8_t spc_in[4], spc_out[4];
static void (*audio_hook)(int16_t l, int16_t r);

static const uint8_t spc_ipl[64] = {
    0xCD, 0xEF, 0xBD, 0xE8, 0x00, 0xC6, 0x1D, 0xD0, 0xFC, 0x8F, 0xAA, 0xF4, 0x8F, 0xBB, 0xF5, 0x78,
    0xCC, 0xF4, 0xD0, 0xFB, 0x2F, 0x19, 0xEB, 0xF4, 0xD0, 0xFC, 0x7E, 0xF4, 0xD0, 0x0B, 0xE4, 0xF5,
    0xCB, 0xF4, 0xD7, 0x00, 0xFC, 0xD0, 0xF3, 0xAB, 0x01, 0x10, 0xEF, 0x7E, 0xF4, 0x10, 0xEB, 0xBA,
    0xF6, 0xDA, 0x00, 0xBA, 0xF4, 0xC4, 0xF4, 0xDD, 0x5D, 0xD0, 0xDB, 0x1F, 0x00, 0x00, 0xC0, 0xFF,
};

/* ---- DSP ---- */

enum { ENV_ATTACK, ENV_DECAY, ENV_SUSTAIN, ENV_RELEASE };

typedef struct {
    int mode, env, cnt, kon_delay, active, pos;
    uint16_t addr;
    int16_t blk[16], hist[4];
    int p1, p2, blk_end, blk_loop;
} Voice;

static uint8_t dsp_reg[128];
static uint8_t dsp_addr;
static Voice voice[8];
static uint8_t kon_latch, endx;
static unsigned dsp_counter = 0x77FF;
static int noise = 0x4000;
static uint16_t echo_pos, echo_len = 4;
static int echo_hist[2][8], echo_idx;
static int gauss[512];
static int gauss_ready;

static const uint16_t rate_period[32] = {0, 2048, 1536, 1280, 1024, 768, 640, 512, 384, 320, 256, 192, 160, 128, 96, 80,
                                         64, 48, 40, 32, 24, 20, 16, 12, 10, 8, 6, 5, 4, 3, 2, 1};

static int rate_hit(int rate) {
    static const unsigned off[3] = {0, 1040, 536};
    if (rate == 0) return 0;
    return (dsp_counter + (rate < 30 ? off[(rate - 1) % 3] : 0)) % rate_period[rate] == 0;
}

static int clamp16(int v) { return v > 32767 ? 32767 : v < -32768 ? -32768 : v; }

static void gauss_init(void) {
    int j;
    if (gauss_ready) return;
    for (j = 0; j < 512; j++) {
        double d = 2.0 - j / 256.0;
        gauss[j] = d < 2.0 ? (int)(1304.0 * exp(-d * d / (2 * 0.3987)) + 0.5) : 0;
    }
    gauss_ready = 1;
}

static void dsp_write(uint8_t r, uint8_t v) {
    if (r & 0x80) return;
    if (r == 0x4C) kon_latch |= v;
    if (r == 0x7C) { endx = 0; v = 0; }
    dsp_reg[r] = v;
}

static uint16_t dir_word(int srcn, int part) {
    uint16_t a = (uint16_t)((dsp_reg[0x5D] << 8) + srcn * 4 + part * 2);
    return (uint16_t)(aram[a] | aram[(uint16_t)(a + 1)] << 8);
}

/* Decodes the 9-byte BRR block at vo->addr; first it follows the end/loop flags of the block that just played. */
static void brr_next(Voice *vo, int v) {
    uint8_t head;
    int i, range, filter;
    if (vo->blk_end) {
        endx |= (uint8_t)(1 << v);
        vo->addr = dir_word(dsp_reg[v * 16 + 4], 1);
        if (!vo->blk_loop) { vo->mode = ENV_RELEASE; vo->env = 0; }
    }
    head = aram[vo->addr];
    range = head >> 4;
    filter = head >> 2 & 3;
    vo->blk_end = head & 1;
    vo->blk_loop = head & 2;
    for (i = 0; i < 16; i++) {
        uint8_t byte = aram[(uint16_t)(vo->addr + 1 + i / 2)];
        int n = i & 1 ? byte & 15 : byte >> 4, s;
        if (n >= 8) n -= 16;
        s = range <= 12 ? (n << range) >> 1 : (n < 0 ? -2048 : 0);
        switch (filter) {
        case 1: s += vo->p1 - (vo->p1 >> 4); break;
        case 2: s += vo->p1 * 2 + ((-vo->p1 * 3) >> 5) - vo->p2 + (vo->p2 >> 4); break;
        case 3: s += vo->p1 * 2 + ((-vo->p1 * 13) >> 6) - vo->p2 + ((vo->p2 * 3) >> 4); break;
        default: break;
        }
        s = clamp16(s);
        s = (int16_t)(s * 2) >> 1; /* the decoder keeps 15 bits */
        vo->blk[i] = (int16_t)s;
        vo->p2 = vo->p1;
        vo->p1 = s;
    }
    vo->addr = (uint16_t)(vo->addr + 9);
    vo->pos = 0;
}

static void env_step(Voice *vo, const uint8_t *r) {
    int adsr1 = r[5], adsr2 = r[6], gain = r[7], env = vo->env, rate;
    if (vo->mode == ENV_RELEASE) {
        vo->env = env > 8 ? env - 8 : 0;
        return;
    }
    if (adsr1 & 0x80) {
        switch (vo->mode) {
        case ENV_ATTACK:
            rate = (adsr1 & 15) * 2 + 1;
            if (!rate_hit(rate)) return;
            env += rate == 31 ? 1024 : 32;
            if (env >= 0x7E0) vo->mode = ENV_DECAY;
            break;
        case ENV_DECAY:
            rate = (adsr1 >> 4 & 7) * 2 + 16;
            if (!rate_hit(rate)) return;
            env -= ((env - 1) >> 8) + 1;
            if ((env >> 8) == (adsr2 >> 5)) vo->mode = ENV_SUSTAIN;
            break;
        default:
            if (!rate_hit(adsr2 & 31)) return;
            env -= ((env - 1) >> 8) + 1;
            break;
        }
    } else if (gain & 0x80) {
        if (!rate_hit(gain & 31)) return;
        switch (gain >> 5 & 3) {
        case 0: env -= 32; break;
        case 1: env -= ((env - 1) >> 8) + 1; break;
        case 2: env += 32; break;
        default: env += env < 0x600 ? 32 : 8; break;
        }
    } else {
        env = (gain & 0x7F) << 4;
    }
    vo->env = env < 0 ? 0 : env > 0x7FF ? 0x7FF : env;
}

static void dsp_sample(void) {
    int mixl = 0, mixr = 0, echol = 0, echor = 0, prev = 0, v, k;
    uint8_t flg = dsp_reg[0x6C];
    dsp_counter = dsp_counter ? dsp_counter - 1 : 0x77FF;
    if (rate_hit(flg & 31)) noise = noise >> 1 | ((noise ^ noise >> 1) & 1) << 14;
    for (v = 0; v < 8; v++) {
        Voice *vo = &voice[v];
        uint8_t *r = &dsp_reg[v * 16], bit = (uint8_t)(1 << v);
        int pitch, s, i, out;
        if (kon_latch & bit) {
            kon_latch &= (uint8_t)~bit;
            endx &= (uint8_t)~bit;
            memset(vo->hist, 0, sizeof vo->hist);
            vo->mode = ENV_ATTACK; vo->env = 0; vo->cnt = 0; vo->kon_delay = 5; vo->active = 1;
            vo->p1 = vo->p2 = 0; vo->blk_end = vo->blk_loop = 0; vo->pos = 16;
            vo->addr = dir_word(r[4], 0);
        }
        if (flg & 0x80) { vo->mode = ENV_RELEASE; vo->env = 0; }
        else if ((dsp_reg[0x5C] & bit) && !vo->kon_delay) vo->mode = ENV_RELEASE;
        if (!vo->active) { r[8] = r[9] = 0; prev = 0; continue; }
        if (vo->kon_delay) {
            vo->kon_delay--;
            r[8] = r[9] = 0;
            prev = 0;
            continue;
        }
        pitch = r[2] | (r[3] & 0x3F) << 8;
        if ((dsp_reg[0x2D] & bit) && v) pitch += ((prev >> 5) * pitch) >> 10;
        pitch = pitch < 0 ? 0 : pitch > 0x3FFF ? 0x3FFF : pitch;
        if (dsp_reg[0x3D] & bit) {
            s = (int16_t)(noise * 2) >> 1;
        } else {
            i = (vo->cnt & 0xFFF) >> 4;
            s = (gauss[255 - i] * vo->hist[0] + gauss[511 - i] * vo->hist[1] + gauss[256 + i] * vo->hist[2] + gauss[i] * vo->hist[3]) >> 11;
            s = clamp16(s);
        }
        env_step(vo, r);
        vo->cnt += pitch;
        while (vo->cnt >= 0x1000) {
            vo->cnt -= 0x1000;
            if (vo->pos >= 16) brr_next(vo, v);
            vo->hist[0] = vo->hist[1]; vo->hist[1] = vo->hist[2]; vo->hist[2] = vo->hist[3];
            vo->hist[3] = vo->blk[vo->pos++];
        }
        out = (s * vo->env) >> 11;
        prev = out;
        r[8] = (uint8_t)(vo->env >> 4);
        r[9] = (uint8_t)(out >> 7);
        mixl = clamp16(mixl + ((out * (int8_t)r[0]) >> 7));
        mixr = clamp16(mixr + ((out * (int8_t)r[1]) >> 7));
        if (dsp_reg[0x4D] & bit) {
            echol = clamp16(echol + ((out * (int8_t)r[0]) >> 7));
            echor = clamp16(echor + ((out * (int8_t)r[1]) >> 7));
        }
        if (vo->mode == ENV_RELEASE && vo->env == 0) vo->active = 0;
    }
    /* echo: read the oldest entry of the buffer, filter the history, mix and write the feedback back */
    {
        uint16_t ea = (uint16_t)((dsp_reg[0x6D] << 8) + echo_pos);
        int inl = (int16_t)(aram[ea] | aram[(uint16_t)(ea + 1)] << 8) >> 1;
        int inr = (int16_t)(aram[(uint16_t)(ea + 2)] | aram[(uint16_t)(ea + 3)] << 8) >> 1;
        int firl = 0, firr = 0, outl, outr;
        echo_hist[0][echo_idx] = inl;
        echo_hist[1][echo_idx] = inr;
        echo_idx = (echo_idx + 1) & 7;
        for (k = 0; k < 8; k++) {
            int c = (int8_t)dsp_reg[k * 16 + 0x0F];
            firl += echo_hist[0][(echo_idx + k) & 7] * c;
            firr += echo_hist[1][(echo_idx + k) & 7] * c;
        }
        firl = clamp16(firl >> 6);
        firr = clamp16(firr >> 6);
        outl = clamp16(((mixl * (int8_t)dsp_reg[0x0C]) >> 7) + ((firl * (int8_t)dsp_reg[0x2C]) >> 7));
        outr = clamp16(((mixr * (int8_t)dsp_reg[0x1C]) >> 7) + ((firr * (int8_t)dsp_reg[0x3C]) >> 7));
        if (!(flg & 0x20)) {
            int wl = clamp16(echol + ((firl * (int8_t)dsp_reg[0x0D]) >> 7)) & ~1;
            int wr = clamp16(echor + ((firr * (int8_t)dsp_reg[0x0D]) >> 7)) & ~1;
            aram[ea] = (uint8_t)wl; aram[(uint16_t)(ea + 1)] = (uint8_t)(wl >> 8);
            aram[(uint16_t)(ea + 2)] = (uint8_t)wr; aram[(uint16_t)(ea + 3)] = (uint8_t)(wr >> 8);
        }
        echo_pos = (uint16_t)(echo_pos + 4);
        if (echo_pos >= echo_len) {
            echo_pos = 0;
            echo_len = (uint16_t)((dsp_reg[0x7D] & 15) ? (dsp_reg[0x7D] & 15) * 2048 : 4);
        }
        if (flg & 0x40) outl = outr = 0;
        if (audio_hook) audio_hook((int16_t)outl, (int16_t)outr);
    }
}

/* ---- SPC700 ---- */

enum { SF_C = 1, SF_Z = 2, SF_I = 4, SF_H = 8, SF_B = 16, SF_P = 32, SF_V = 64, SF_N = 128 };

typedef struct { uint8_t target, out; uint16_t stage0, acc; int on; } SpcTimer;

static uint8_t spc_a, spc_x, spc_y, spc_sp, spc_psw, spc_f1 = 0x80, spc_halted;
static uint16_t spc_pc;
static SpcTimer spc_timer[3];
static unsigned dsp_acc;
static int64_t spc_budget;

static uint8_t spc_read(uint16_t a) {
    if (a >= 0xF0 && a < 0x100) {
        switch (a) {
        case 0xF2: return dsp_addr;
        case 0xF3: return (dsp_addr & 0x7F) == 0x7C ? endx : dsp_reg[dsp_addr & 0x7F];
        case 0xF4: case 0xF5: case 0xF6: case 0xF7: return spc_in[a - 0xF4];
        case 0xFD: case 0xFE: case 0xFF: {
            SpcTimer *t = &spc_timer[a - 0xFD];
            uint8_t o = t->out;
            t->out = 0;
            return o;
        }
        default: return aram[a];
        }
    }
    if (a >= 0xFFC0 && (spc_f1 & 0x80)) return spc_ipl[a - 0xFFC0];
    return aram[a];
}

static void spc_write(uint16_t a, uint8_t v) {
    if (a >= 0xF0 && a < 0x100) {
        switch (a) {
        case 0xF1: {
            int i;
            for (i = 0; i < 3; i++) {
                int on = v >> i & 1;
                if (on && !spc_timer[i].on) { spc_timer[i].stage0 = 0; spc_timer[i].out = 0; }
                spc_timer[i].on = on;
            }
            if (v & 0x10) spc_in[0] = spc_in[1] = 0;
            if (v & 0x20) spc_in[2] = spc_in[3] = 0;
            spc_f1 = v;
            break;
        }
        case 0xF2: dsp_addr = v; break;
        case 0xF3: dsp_write(dsp_addr, v); break;
        case 0xF4: case 0xF5: case 0xF6: case 0xF7: spc_out[a - 0xF4] = v; break;
        case 0xFA: case 0xFB: case 0xFC: spc_timer[a - 0xFA].target = v; break;
        default: break;
        }
    }
    aram[a] = v;
}

static uint8_t spc_imm(void) { return spc_read(spc_pc++); }
static uint16_t spc_imm16(void) { uint8_t lo = spc_imm(); return (uint16_t)(lo | spc_imm() << 8); }
static uint16_t spc_dp(uint8_t o) { return (uint16_t)((spc_psw & SF_P ? 0x100 : 0) | o); }
static uint16_t spc_dp16(uint8_t o) { /* the 16-bit value at a direct page offset (the high byte wraps in the page) */
    uint8_t lo = spc_read(spc_dp(o));
    return (uint16_t)(lo | spc_read(spc_dp((uint8_t)(o + 1))) << 8);
}
static void spc_push(uint8_t v) { spc_write((uint16_t)(0x100 | spc_sp--), v); }
static uint8_t spc_pop(void) { return spc_read((uint16_t)(0x100 | ++spc_sp)); }
static void spc_setf(int f, int on) { if (on) spc_psw |= (uint8_t)f; else spc_psw &= (uint8_t)~f; }
static uint8_t spc_nz(uint8_t v) { spc_setf(SF_N, v & 0x80); spc_setf(SF_Z, v == 0); return v; }
static void spc_nz16(unsigned v) { spc_setf(SF_N, v & 0x8000); spc_setf(SF_Z, (v & 0xFFFF) == 0); }

static uint8_t spc_alu(int kind, uint8_t a, uint8_t b) {
    unsigned r, c = spc_psw & SF_C;
    switch (kind) {
    case 0: return spc_nz((uint8_t)(a | b));
    case 1: return spc_nz((uint8_t)(a & b));
    case 2: return spc_nz((uint8_t)(a ^ b));
    case 3: spc_setf(SF_C, a >= b); spc_nz((uint8_t)(a - b)); return a;
    default:
        if (kind == 5) b ^= 0xFF;
        r = (unsigned)a + b + c;
        spc_setf(SF_H, (a & 15) + (b & 15) + c > 15);
        spc_setf(SF_V, ~(a ^ b) & (a ^ r) & 0x80);
        spc_setf(SF_C, r > 0xFF);
        return spc_nz((uint8_t)r);
    }
}

static uint8_t spc_shift(int kind, uint8_t v) { /* 0 ASL, 1 ROL, 2 LSR, 3 ROR */
    unsigned c = spc_psw & SF_C, out;
    switch (kind) {
    case 0: out = (unsigned)v << 1; spc_setf(SF_C, v & 0x80); break;
    case 1: out = (unsigned)v << 1 | c; spc_setf(SF_C, v & 0x80); break;
    case 2: out = v >> 1; spc_setf(SF_C, v & 1); break;
    default: out = v >> 1 | c << 7; spc_setf(SF_C, v & 1); break;
    }
    return spc_nz((uint8_t)out);
}

static const uint8_t spc_cycles[256] = {
    2, 8, 4, 5, 3, 4, 3, 6, 2, 6, 5, 4, 5, 4, 6, 8,
    2, 8, 4, 5, 4, 5, 5, 6, 5, 5, 6, 5, 2, 2, 4, 6,
    2, 8, 4, 5, 3, 4, 3, 6, 2, 6, 5, 4, 5, 4, 5, 4,
    2, 8, 4, 5, 4, 5, 5, 6, 5, 5, 6, 5, 2, 2, 3, 8,
    2, 8, 4, 5, 3, 4, 3, 6, 2, 6, 4, 4, 5, 4, 6, 6,
    2, 8, 4, 5, 4, 5, 5, 6, 5, 5, 4, 5, 2, 2, 4, 3,
    2, 8, 4, 5, 3, 4, 3, 6, 2, 6, 4, 4, 5, 4, 5, 5,
    2, 8, 4, 5, 4, 5, 5, 6, 5, 5, 5, 5, 2, 2, 3, 6,
    2, 8, 4, 5, 3, 4, 3, 6, 2, 6, 5, 4, 5, 2, 4, 5,
    2, 8, 4, 5, 4, 5, 5, 6, 5, 5, 5, 5, 2, 2, 12, 5,
    3, 8, 4, 5, 3, 4, 3, 6, 2, 6, 4, 4, 5, 2, 4, 4,
    2, 8, 4, 5, 4, 5, 5, 6, 5, 5, 5, 5, 2, 2, 3, 4,
    3, 8, 4, 5, 4, 5, 4, 7, 2, 5, 6, 4, 5, 2, 4, 9,
    2, 8, 4, 5, 5, 6, 6, 7, 4, 5, 5, 5, 2, 2, 6, 3,
    2, 8, 4, 5, 3, 4, 3, 6, 2, 4, 5, 3, 4, 3, 4, 3,
    2, 8, 4, 5, 4, 5, 5, 6, 3, 4, 5, 4, 2, 2, 4, 3,
};

/* Runs one instruction and returns its cycles. */
static int spc_step(void) {
    uint8_t op = spc_imm(), lo = op & 15, hi = op >> 4, d, e, v, rel;
    int cyc = spc_cycles[op], kind, taken = 0;
    uint16_t ea, w, m;
    unsigned u;

    if (hi <= 0xB && lo >= 4 && lo <= 9) { /* OR AND EOR CMP ADC SBC */
        kind = hi >> 1;
        if (hi & 1 ? lo >= 8 : lo == 9) { /* the forms that write memory: dd,ds / d,#i / (X),(Y) */
            uint8_t src;
            uint16_t dst;
            if (hi & 1 && lo == 8) { src = spc_imm(); dst = spc_dp(spc_imm()); }
            else if (hi & 1) { src = spc_read(spc_dp(spc_y)); dst = spc_dp(spc_x); }
            else { src = spc_read(spc_dp(spc_imm())); dst = spc_dp(spc_imm()); }
            v = spc_alu(kind, spc_read(dst), src);
            if (kind != 3) spc_write(dst, v);
            return cyc;
        }
        if (hi & 1) {
            switch (lo) {
            case 4: ea = spc_dp((uint8_t)(spc_imm() + spc_x)); break;
            case 5: ea = (uint16_t)(spc_imm16() + spc_x); break;
            case 6: ea = (uint16_t)(spc_imm16() + spc_y); break;
            default: ea = (uint16_t)(spc_dp16(spc_imm()) + spc_y); break;
            }
        } else {
            switch (lo) {
            case 4: ea = spc_dp(spc_imm()); break;
            case 5: ea = spc_imm16(); break;
            case 6: ea = spc_dp(spc_x); break;
            case 7: ea = spc_dp16((uint8_t)(spc_imm() + spc_x)); break;
            default: ea = spc_pc++; break;
            }
        }
        spc_a = spc_alu(kind, spc_a, spc_read(ea));
        return cyc;
    }

    switch (op) {
    case 0x00: case 0xEF: case 0xFF:
        if (op != 0) { spc_halted = 1; spc_pc--; }
        break;
    /* branches */
    case 0x10: case 0x30: case 0x50: case 0x70: case 0x90: case 0xB0: case 0xD0: case 0xF0: case 0x2F:
        rel = spc_imm();
        switch (op) {
        case 0x10: taken = !(spc_psw & SF_N); break;
        case 0x30: taken = spc_psw & SF_N; break;
        case 0x50: taken = !(spc_psw & SF_V); break;
        case 0x70: taken = spc_psw & SF_V; break;
        case 0x90: taken = !(spc_psw & SF_C); break;
        case 0xB0: taken = spc_psw & SF_C; break;
        case 0xD0: taken = !(spc_psw & SF_Z); break;
        case 0xF0: taken = spc_psw & SF_Z; break;
        default: taken = 1; break;
        }
        if (taken) { spc_pc = (uint16_t)(spc_pc + (int8_t)rel); cyc += op == 0x2F ? 0 : 2; }
        break;
    case 0x03: case 0x23: case 0x43: case 0x63: case 0x83: case 0xA3: case 0xC3: case 0xE3: /* BBS */
    case 0x13: case 0x33: case 0x53: case 0x73: case 0x93: case 0xB3: case 0xD3: case 0xF3: { /* BBC */
        d = spc_imm();
        rel = spc_imm();
        taken = (spc_read(spc_dp(d)) >> (op >> 5) & 1) == !(op & 0x10);
        if (taken) { spc_pc = (uint16_t)(spc_pc + (int8_t)rel); cyc += 2; }
        break;
    }
    case 0x02: case 0x22: case 0x42: case 0x62: case 0x82: case 0xA2: case 0xC2: case 0xE2: /* SET1 */
    case 0x12: case 0x32: case 0x52: case 0x72: case 0x92: case 0xB2: case 0xD2: case 0xF2: /* CLR1 */
        ea = spc_dp(spc_imm());
        v = spc_read(ea);
        spc_write(ea, op & 0x10 ? (uint8_t)(v & ~(1 << (op >> 5))) : (uint8_t)(v | 1 << (op >> 5)));
        break;
    case 0x2E: case 0xDE: /* CBNE */
        d = spc_imm();
        rel = spc_imm();
        v = spc_read(op == 0xDE ? spc_dp((uint8_t)(d + spc_x)) : spc_dp(d));
        if (spc_a != v) { spc_pc = (uint16_t)(spc_pc + (int8_t)rel); cyc += 2; }
        break;
    case 0x6E: /* DBNZ d */
        d = spc_imm();
        rel = spc_imm();
        ea = spc_dp(d);
        v = (uint8_t)(spc_read(ea) - 1);
        spc_write(ea, v);
        if (v) { spc_pc = (uint16_t)(spc_pc + (int8_t)rel); cyc += 2; }
        break;
    case 0xFE: /* DBNZ Y */
        rel = spc_imm();
        if (--spc_y) { spc_pc = (uint16_t)(spc_pc + (int8_t)rel); cyc += 2; }
        break;
    case 0x5F: spc_pc = spc_imm16(); break;
    case 0x1F: ea = (uint16_t)(spc_imm16() + spc_x); spc_pc = (uint16_t)(spc_read(ea) | spc_read((uint16_t)(ea + 1)) << 8); break;
    case 0x3F: w = spc_imm16(); spc_push((uint8_t)(spc_pc >> 8)); spc_push((uint8_t)spc_pc); spc_pc = w; break;
    case 0x4F: d = spc_imm(); spc_push((uint8_t)(spc_pc >> 8)); spc_push((uint8_t)spc_pc); spc_pc = (uint16_t)(0xFF00 | d); break;
    case 0x01: case 0x11: case 0x21: case 0x31: case 0x41: case 0x51: case 0x61: case 0x71:
    case 0x81: case 0x91: case 0xA1: case 0xB1: case 0xC1: case 0xD1: case 0xE1: case 0xF1:
        spc_push((uint8_t)(spc_pc >> 8)); spc_push((uint8_t)spc_pc);
        ea = (uint16_t)(0xFFDE - 2 * (op >> 4));
        spc_pc = (uint16_t)(spc_read(ea) | spc_read((uint16_t)(ea + 1)) << 8);
        break;
    case 0x0F:
        spc_push((uint8_t)(spc_pc >> 8)); spc_push((uint8_t)spc_pc); spc_push(spc_psw);
        spc_setf(SF_B, 1); spc_setf(SF_I, 0);
        spc_pc = (uint16_t)(spc_read(0xFFDE) | spc_read(0xFFDF) << 8);
        break;
    case 0x6F: d = spc_pop(); spc_pc = (uint16_t)(d | spc_pop() << 8); break;
    case 0x7F: spc_psw = spc_pop(); d = spc_pop(); spc_pc = (uint16_t)(d | spc_pop() << 8); break;
    /* stack */
    case 0x2D: spc_push(spc_a); break;
    case 0x4D: spc_push(spc_x); break;
    case 0x6D: spc_push(spc_y); break;
    case 0x0D: spc_push(spc_psw); break;
    case 0xAE: spc_a = spc_pop(); break;
    case 0xCE: spc_x = spc_pop(); break;
    case 0xEE: spc_y = spc_pop(); break;
    case 0x8E: spc_psw = spc_pop(); break;
    /* flags */
    case 0x20: spc_setf(SF_P, 0); break;
    case 0x40: spc_setf(SF_P, 1); break;
    case 0x60: spc_setf(SF_C, 0); break;
    case 0x80: spc_setf(SF_C, 1); break;
    case 0xA0: spc_setf(SF_I, 1); break;
    case 0xC0: spc_setf(SF_I, 0); break;
    case 0xE0: spc_setf(SF_V, 0); spc_setf(SF_H, 0); break;
    case 0xED: spc_psw ^= SF_C; break;
    /* single bit operations on the carry */
    case 0x0A: case 0x2A: case 0x4A: case 0x6A: case 0x8A: case 0xAA: case 0xCA: case 0xEA:
        m = spc_imm16();
        ea = m & 0x1FFF;
        v = spc_read(ea);
        e = v >> (m >> 13) & 1;
        switch (op) {
        case 0x0A: if (e) spc_setf(SF_C, 1); break;
        case 0x2A: if (!e) spc_setf(SF_C, 1); break;
        case 0x4A: if (!e) spc_setf(SF_C, 0); break;
        case 0x6A: if (e) spc_setf(SF_C, 0); break;
        case 0x8A: if (e) spc_psw ^= SF_C; break;
        case 0xAA: spc_setf(SF_C, e); break;
        case 0xCA: spc_write(ea, spc_psw & SF_C ? (uint8_t)(v | 1 << (m >> 13)) : (uint8_t)(v & ~(1 << (m >> 13)))); break;
        default: spc_write(ea, (uint8_t)(v ^ 1 << (m >> 13))); break;
        }
        break;
    /* shifts and rotates */
    case 0x1C: spc_a = spc_shift(0, spc_a); break;
    case 0x3C: spc_a = spc_shift(1, spc_a); break;
    case 0x5C: spc_a = spc_shift(2, spc_a); break;
    case 0x7C: spc_a = spc_shift(3, spc_a); break;
    case 0x0B: case 0x2B: case 0x4B: case 0x6B: ea = spc_dp(spc_imm()); spc_write(ea, spc_shift(op >> 5, spc_read(ea))); break;
    case 0x1B: case 0x3B: case 0x5B: case 0x7B: ea = spc_dp((uint8_t)(spc_imm() + spc_x)); spc_write(ea, spc_shift(op >> 5, spc_read(ea))); break;
    case 0x0C: case 0x2C: case 0x4C: case 0x6C: ea = spc_imm16(); spc_write(ea, spc_shift(op >> 5, spc_read(ea))); break;
    /* increments and decrements */
    case 0xBC: spc_a = spc_nz((uint8_t)(spc_a + 1)); break;
    case 0x9C: spc_a = spc_nz((uint8_t)(spc_a - 1)); break;
    case 0x3D: spc_x = spc_nz((uint8_t)(spc_x + 1)); break;
    case 0x1D: spc_x = spc_nz((uint8_t)(spc_x - 1)); break;
    case 0xFC: spc_y = spc_nz((uint8_t)(spc_y + 1)); break;
    case 0xDC: spc_y = spc_nz((uint8_t)(spc_y - 1)); break;
    case 0xAB: ea = spc_dp(spc_imm()); spc_write(ea, spc_nz((uint8_t)(spc_read(ea) + 1))); break;
    case 0x8B: ea = spc_dp(spc_imm()); spc_write(ea, spc_nz((uint8_t)(spc_read(ea) - 1))); break;
    case 0xBB: ea = spc_dp((uint8_t)(spc_imm() + spc_x)); spc_write(ea, spc_nz((uint8_t)(spc_read(ea) + 1))); break;
    case 0x9B: ea = spc_dp((uint8_t)(spc_imm() + spc_x)); spc_write(ea, spc_nz((uint8_t)(spc_read(ea) - 1))); break;
    case 0xAC: ea = spc_imm16(); spc_write(ea, spc_nz((uint8_t)(spc_read(ea) + 1))); break;
    case 0x8C: ea = spc_imm16(); spc_write(ea, spc_nz((uint8_t)(spc_read(ea) - 1))); break;
    /* compare X and Y */
    case 0xC8: spc_alu(3, spc_x, spc_imm()); break;
    case 0x3E: spc_alu(3, spc_x, spc_read(spc_dp(spc_imm()))); break;
    case 0x1E: spc_alu(3, spc_x, spc_read(spc_imm16())); break;
    case 0xAD: spc_alu(3, spc_y, spc_imm()); break;
    case 0x7E: spc_alu(3, spc_y, spc_read(spc_dp(spc_imm()))); break;
    case 0x5E: spc_alu(3, spc_y, spc_read(spc_imm16())); break;
    /* loads */
    case 0xE8: spc_a = spc_nz(spc_imm()); break;
    case 0xE6: spc_a = spc_nz(spc_read(spc_dp(spc_x))); break;
    case 0xBF: spc_a = spc_nz(spc_read(spc_dp(spc_x))); spc_x++; break;
    case 0xE4: spc_a = spc_nz(spc_read(spc_dp(spc_imm()))); break;
    case 0xF4: spc_a = spc_nz(spc_read(spc_dp((uint8_t)(spc_imm() + spc_x)))); break;
    case 0xE5: spc_a = spc_nz(spc_read(spc_imm16())); break;
    case 0xF5: spc_a = spc_nz(spc_read((uint16_t)(spc_imm16() + spc_x))); break;
    case 0xF6: spc_a = spc_nz(spc_read((uint16_t)(spc_imm16() + spc_y))); break;
    case 0xE7: spc_a = spc_nz(spc_read(spc_dp16((uint8_t)(spc_imm() + spc_x)))); break;
    case 0xF7: spc_a = spc_nz(spc_read((uint16_t)(spc_dp16(spc_imm()) + spc_y))); break;
    case 0xCD: spc_x = spc_nz(spc_imm()); break;
    case 0xF8: spc_x = spc_nz(spc_read(spc_dp(spc_imm()))); break;
    case 0xF9: spc_x = spc_nz(spc_read(spc_dp((uint8_t)(spc_imm() + spc_y)))); break;
    case 0xE9: spc_x = spc_nz(spc_read(spc_imm16())); break;
    case 0x8D: spc_y = spc_nz(spc_imm()); break;
    case 0xEB: spc_y = spc_nz(spc_read(spc_dp(spc_imm()))); break;
    case 0xFB: spc_y = spc_nz(spc_read(spc_dp((uint8_t)(spc_imm() + spc_x)))); break;
    case 0xEC: spc_y = spc_nz(spc_read(spc_imm16())); break;
    /* stores */
    case 0xC4: spc_write(spc_dp(spc_imm()), spc_a); break;
    case 0xD4: spc_write(spc_dp((uint8_t)(spc_imm() + spc_x)), spc_a); break;
    case 0xC5: spc_write(spc_imm16(), spc_a); break;
    case 0xD5: spc_write((uint16_t)(spc_imm16() + spc_x), spc_a); break;
    case 0xD6: spc_write((uint16_t)(spc_imm16() + spc_y), spc_a); break;
    case 0xC6: spc_write(spc_dp(spc_x), spc_a); break;
    case 0xAF: spc_write(spc_dp(spc_x), spc_a); spc_x++; break;
    case 0xC7: spc_write(spc_dp16((uint8_t)(spc_imm() + spc_x)), spc_a); break;
    case 0xD7: spc_write((uint16_t)(spc_dp16(spc_imm()) + spc_y), spc_a); break;
    case 0xD8: spc_write(spc_dp(spc_imm()), spc_x); break;
    case 0xD9: spc_write(spc_dp((uint8_t)(spc_imm() + spc_y)), spc_x); break;
    case 0xC9: spc_write(spc_imm16(), spc_x); break;
    case 0xCB: spc_write(spc_dp(spc_imm()), spc_y); break;
    case 0xDB: spc_write(spc_dp((uint8_t)(spc_imm() + spc_x)), spc_y); break;
    case 0xCC: spc_write(spc_imm16(), spc_y); break;
    case 0x8F: v = spc_imm(); spc_write(spc_dp(spc_imm()), v); break;
    case 0xFA: v = spc_read(spc_dp(spc_imm())); spc_write(spc_dp(spc_imm()), v); break;
    /* register moves */
    case 0x5D: spc_x = spc_nz(spc_a); break;
    case 0x7D: spc_a = spc_nz(spc_x); break;
    case 0xDD: spc_a = spc_nz(spc_y); break;
    case 0xFD: spc_y = spc_nz(spc_a); break;
    case 0x9D: spc_x = spc_nz(spc_sp); break;
    case 0xBD: spc_sp = spc_x; break;
    /* test and set / clear */
    case 0x0E: case 0x4E:
        ea = spc_imm16();
        v = spc_read(ea);
        spc_nz((uint8_t)(spc_a - v));
        spc_write(ea, op == 0x0E ? (uint8_t)(v | spc_a) : (uint8_t)(v & ~spc_a));
        break;
    /* 16 bit */
    case 0xBA: d = spc_imm(); u = spc_dp16(d); spc_a = (uint8_t)u; spc_y = (uint8_t)(u >> 8); spc_nz16(u); break;
    case 0xDA:
        d = spc_imm();
        spc_write(spc_dp(d), spc_a);
        spc_write(spc_dp((uint8_t)(d + 1)), spc_y);
        break;
    case 0x7A: case 0x9A: {
        unsigned ya = spc_a | spc_y << 8, o = spc_dp16(spc_imm()), r;
        if (op == 0x7A) { r = ya + o; spc_setf(SF_V, ~(ya ^ o) & (ya ^ r) & 0x8000); spc_setf(SF_H, (ya & 0xFFF) + (o & 0xFFF) > 0xFFF); spc_setf(SF_C, r > 0xFFFF); }
        else { r = ya - o; spc_setf(SF_V, (ya ^ o) & (ya ^ r) & 0x8000); spc_setf(SF_H, (ya & 0xFFF) >= (o & 0xFFF)); spc_setf(SF_C, ya >= o); }
        spc_a = (uint8_t)r;
        spc_y = (uint8_t)(r >> 8);
        spc_nz16(r);
        break;
    }
    case 0x5A: {
        unsigned ya = spc_a | spc_y << 8, o = spc_dp16(spc_imm());
        spc_setf(SF_C, ya >= o);
        spc_nz16(ya - o);
        break;
    }
    case 0x3A: case 0x1A: {
        d = spc_imm();
        u = (spc_dp16(d) + (op == 0x3A ? 1 : 0xFFFF)) & 0xFFFF;
        spc_write(spc_dp(d), (uint8_t)u);
        spc_write(spc_dp((uint8_t)(d + 1)), (uint8_t)(u >> 8));
        spc_nz16(u);
        break;
    }
    case 0xCF: u = (unsigned)spc_y * spc_a; spc_a = (uint8_t)u; spc_y = (uint8_t)(u >> 8); spc_nz(spc_y); break;
    case 0x9E: {
        unsigned ya = spc_a | spc_y << 8, q, rem;
        spc_setf(SF_V, spc_y >= spc_x);
        spc_setf(SF_H, (spc_y & 15) >= (spc_x & 15));
        if (spc_y < (spc_x << 1)) {
            q = spc_x ? ya / spc_x : 0xFF;
            rem = spc_x ? ya % spc_x : spc_a;
        } else {
            unsigned t = ya - ((unsigned)spc_x << 9);
            q = 255 - t / (256 - spc_x);
            rem = spc_x + t % (256 - spc_x);
        }
        spc_a = (uint8_t)q;
        spc_y = (uint8_t)rem;
        spc_nz(spc_a);
        break;
    }
    case 0xDF:
        if ((spc_psw & SF_C) || spc_a > 0x99) { spc_a = (uint8_t)(spc_a + 0x60); spc_setf(SF_C, 1); }
        if ((spc_psw & SF_H) || (spc_a & 15) > 9) spc_a = (uint8_t)(spc_a + 6);
        spc_nz(spc_a);
        break;
    case 0xBE:
        if (!(spc_psw & SF_C) || spc_a > 0x99) { spc_a = (uint8_t)(spc_a - 0x60); spc_setf(SF_C, 0); }
        if (!(spc_psw & SF_H) || (spc_a & 15) > 9) spc_a = (uint8_t)(spc_a - 6);
        spc_nz(spc_a);
        break;
    case 0x9F: spc_a = spc_nz((uint8_t)(spc_a << 4 | spc_a >> 4)); break;
    default: break;
    }
    return cyc;
}

static void spc_reset(void) {
    memset(aram, 0, sizeof aram);
    memset(dsp_reg, 0, sizeof dsp_reg);
    memset(voice, 0, sizeof voice);
    memset(spc_timer, 0, sizeof spc_timer);
    memset(spc_in, 0, sizeof spc_in);
    memset(spc_out, 0, sizeof spc_out);
    spc_a = spc_x = spc_y = spc_sp = spc_psw = 0;
    spc_f1 = 0x80;
    spc_halted = 0;
    spc_pc = 0xFFC0;
    dsp_reg[0x6C] = 0xE0;
    kon_latch = endx = 0;
    dsp_counter = 0x77FF;
    noise = 0x4000;
    echo_pos = 0; echo_len = 4;
    memset(echo_hist, 0, sizeof echo_hist);
    dsp_acc = 0;
    spc_budget = 0;
    gauss_init();
}

/* One tick of the frame clock is 17039 / frame_len SPC700 cycles; the remainder carries over in spc_budget. */
static void spc_advance(uint32_t ticks) {
    spc_budget += (int64_t)17039 * ticks;
    while (spc_budget > 0) {
        int c = spc_step(), i;
        spc_budget -= (int64_t)c * frame_len;
        for (i = 0; i < 3; i++) {
            SpcTimer *t = &spc_timer[i];
            unsigned div = i == 2 ? 16 : 128;
            if (!t->on) continue;
            t->acc = (uint16_t)(t->acc + c);
            while (t->acc >= div) {
                t->acc = (uint16_t)(t->acc - div);
                if (++t->stage0 == (t->target ? t->target : 256u)) {
                    t->stage0 = 0;
                    t->out = (uint8_t)((t->out + 1) & 15);
                }
            }
        }
        dsp_acc += (unsigned)c;
        while (dsp_acc >= 32) {
            dsp_acc -= 32;
            dsp_sample();
        }
    }
}
