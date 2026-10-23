/* snes_rt.h - interface between recompiled 65816 code and the rest of the system.
 *
 * The generated C only touches the outside world through what is declared here:
 * the CPU state, the bus functions and the runtime hooks. A real runtime provides
 * bus_read8/bus_write8 and the hooks; everything else is defined in this header.
 *
 * Known limits: the stack is not confined to page 1 in emulation mode, and the
 * direct page is not wrapped at 8 bits in emulation mode.
 */
#ifndef SNES_RT_H
#define SNES_RT_H

#include <stdint.h>

typedef struct Cpu {
    uint16_t a, x, y, s, d;
    uint8_t db;
    /* Flags, each 0 or 1. */
    uint8_t c, z, v, n, dec, irq;
    uint8_t e;  /* emulation mode */
    uint8_t m;  /* 1: 8-bit accumulator and memory */
    uint8_t xf; /* 1: 8-bit index registers */
} Cpu;

/* ---- Provided by the runtime ---- */

uint8_t bus_read8(uint32_t addr);
void bus_write8(uint32_t addr, uint8_t value);

/* An indirect jump or call reached a target the recompiler did not know about. */
void rt_unresolved(Cpu *c, uint32_t site, uint32_t target);
/* A block was entered with M/X different from what the recompiler assumed. */
void rt_flag_mismatch(Cpu *c, uint32_t addr);
void rt_wai(Cpu *c);
void rt_stp(Cpu *c);
void rt_brk(Cpu *c);
void rt_cop(Cpu *c);
void rt_wdm(Cpu *c);

#ifdef SNES_RT_CHECKS
static inline void rt_check_flags(Cpu *c, uint32_t addr, uint8_t m, uint8_t x) {
    if ((m != 2 && c->m != m) || (x != 2 && c->xf != x)) rt_flag_mismatch(c, addr);
}
#else
#define rt_check_flags(c, addr, m, x) ((void)0)
#endif

/* ---- Bus access by size ----
 * The plain versions carry on into the next bank; the _b0 versions are for the
 * direct page and the stack and wrap inside bank 0. */

static inline uint16_t bus_read16(uint32_t a) {
    return (uint16_t)(bus_read8(a & 0xFFFFFF) | (bus_read8((a + 1) & 0xFFFFFF) << 8));
}
static inline void bus_write16(uint32_t a, uint16_t v) {
    bus_write8(a & 0xFFFFFF, (uint8_t)v);
    bus_write8((a + 1) & 0xFFFFFF, (uint8_t)(v >> 8));
}
static inline uint16_t bus_read16_b0(uint32_t a) {
    a &= 0xFFFF;
    return (uint16_t)(bus_read8(a) | (bus_read8((a + 1) & 0xFFFF) << 8));
}
static inline void bus_write16_b0(uint32_t a, uint16_t v) {
    a &= 0xFFFF;
    bus_write8(a, (uint8_t)v);
    bus_write8((a + 1) & 0xFFFF, (uint8_t)(v >> 8));
}
static inline uint32_t bus_read24_b0(uint32_t a) {
    a &= 0xFFFF;
    return (uint32_t)bus_read8(a) | ((uint32_t)bus_read8((a + 1) & 0xFFFF) << 8) |
           ((uint32_t)bus_read8((a + 2) & 0xFFFF) << 16);
}

/* ---- Flags ---- */

static inline void rt_nz8(Cpu *c, uint8_t v) { c->z = v == 0; c->n = v >> 7; }
static inline void rt_nz16(Cpu *c, uint16_t v) { c->z = v == 0; c->n = v >> 15; }

static inline uint8_t rt_get_p(const Cpu *c) {
    return (uint8_t)(c->n << 7 | c->v << 6 | c->m << 5 | c->xf << 4 | c->dec << 3 | c->irq << 2 |
                     c->z << 1 | c->c);
}
static inline void rt_set_p(Cpu *c, uint8_t p) {
    c->n = p >> 7 & 1; c->v = p >> 6 & 1; c->dec = p >> 3 & 1;
    c->irq = p >> 2 & 1; c->z = p >> 1 & 1; c->c = p & 1;
    if (!c->e) { c->m = p >> 5 & 1; c->xf = p >> 4 & 1; }
    if (c->xf) { c->x &= 0xFF; c->y &= 0xFF; }
}
static inline void rt_rep(Cpu *c, uint8_t mask) { rt_set_p(c, (uint8_t)(rt_get_p(c) & ~mask)); }
static inline void rt_sep(Cpu *c, uint8_t mask) { rt_set_p(c, (uint8_t)(rt_get_p(c) | mask)); }

static inline void rt_xce(Cpu *c) {
    uint8_t t = c->c;
    c->c = c->e;
    c->e = t;
    if (c->e) {
        c->m = 1; c->xf = 1;
        c->x &= 0xFF; c->y &= 0xFF;
        c->s = (uint16_t)(0x100 | (c->s & 0xFF));
    }
}

/* ---- Stack (bank 0) ---- */

static inline void rt_push8(Cpu *c, uint8_t v) { bus_write8(c->s, v); c->s--; }
static inline void rt_push16(Cpu *c, uint16_t v) { rt_push8(c, (uint8_t)(v >> 8)); rt_push8(c, (uint8_t)v); }
static inline uint8_t rt_pull8(Cpu *c) { c->s++; return bus_read8(c->s); }
static inline uint16_t rt_pull16(Cpu *c) {
    uint16_t lo = rt_pull8(c);
    return (uint16_t)(lo | (rt_pull8(c) << 8));
}
/* RTI: status, then the return address (and the program bank in native mode). */
static inline void rt_rti(Cpu *c) {
    rt_set_p(c, rt_pull8(c));
    (void)rt_pull16(c);
    if (!c->e) (void)rt_pull8(c);
}

/* ---- Loads and logic ---- */

static inline void rt_lda8(Cpu *c, uint8_t v) { c->a = (uint16_t)((c->a & 0xFF00) | v); rt_nz8(c, v); }
static inline void rt_lda16(Cpu *c, uint16_t v) { c->a = v; rt_nz16(c, v); }
static inline void rt_ldx8(Cpu *c, uint8_t v) { c->x = v; rt_nz8(c, v); }
static inline void rt_ldx16(Cpu *c, uint16_t v) { c->x = v; rt_nz16(c, v); }
static inline void rt_ldy8(Cpu *c, uint8_t v) { c->y = v; rt_nz8(c, v); }
static inline void rt_ldy16(Cpu *c, uint16_t v) { c->y = v; rt_nz16(c, v); }

static inline void rt_and8(Cpu *c, uint8_t v) { rt_lda8(c, (uint8_t)(c->a & v)); }
static inline void rt_and16(Cpu *c, uint16_t v) { rt_lda16(c, (uint16_t)(c->a & v)); }
static inline void rt_ora8(Cpu *c, uint8_t v) { rt_lda8(c, (uint8_t)(c->a | v)); }
static inline void rt_ora16(Cpu *c, uint16_t v) { rt_lda16(c, (uint16_t)(c->a | v)); }
static inline void rt_eor8(Cpu *c, uint8_t v) { rt_lda8(c, (uint8_t)(c->a ^ v)); }
static inline void rt_eor16(Cpu *c, uint16_t v) { rt_lda16(c, (uint16_t)(c->a ^ v)); }

static inline void rt_bit8(Cpu *c, uint8_t v) {
    c->z = (c->a & v) == 0; c->n = v >> 7; c->v = v >> 6 & 1;
}
static inline void rt_bit16(Cpu *c, uint16_t v) {
    c->z = (c->a & v) == 0; c->n = v >> 15; c->v = v >> 14 & 1;
}
/* BIT #imm only affects Z. */
static inline void rt_bit_imm8(Cpu *c, uint8_t v) { c->z = ((uint8_t)c->a & v) == 0; }
static inline void rt_bit_imm16(Cpu *c, uint16_t v) { c->z = (c->a & v) == 0; }

static inline void rt_cmp8(Cpu *c, uint8_t reg, uint8_t v) {
    c->c = reg >= v;
    rt_nz8(c, (uint8_t)(reg - v));
}
static inline void rt_cmp16(Cpu *c, uint16_t reg, uint16_t v) {
    c->c = reg >= v;
    rt_nz16(c, (uint16_t)(reg - v));
}

/* ---- Arithmetic, binary and decimal ---- */

static inline uint16_t rt_adc_core(Cpu *c, uint32_t a, uint32_t v, int bits) {
    uint32_t mask = bits == 8 ? 0xFFu : 0xFFFFu;
    uint32_t sign = bits == 8 ? 0x80u : 0x8000u;
    uint32_t r;
    if (!c->dec) {
        r = a + v + c->c;
        c->v = ((~(a ^ v)) & (a ^ r) & sign) != 0;
        c->c = r > mask;
    } else {
        int top = bits / 4 - 1, i;
        uint32_t carry = c->c, t;
        r = 0;
        for (i = 0; i < top; i++) {
            t = ((a >> (4 * i)) & 0xF) + ((v >> (4 * i)) & 0xF) + carry;
            if (t > 9) t += 6;
            carry = t > 0xF;
            r |= (t & 0xF) << (4 * i);
        }
        t = ((a >> (4 * top)) & 0xF) + ((v >> (4 * top)) & 0xF) + carry;
        r |= t << (4 * top);
        c->v = ((~(a ^ v)) & (a ^ r) & sign) != 0;
        if (t > 9) r += 6u << (4 * top);
        c->c = r > mask;
    }
    return (uint16_t)(r & mask);
}

static inline uint16_t rt_sbc_core(Cpu *c, uint32_t a, uint32_t v, int bits) {
    uint32_t mask = bits == 8 ? 0xFFu : 0xFFFFu;
    uint32_t sign = bits == 8 ? 0x80u : 0x8000u;
    int32_t r;
    v = ~v & mask;
    if (!c->dec) {
        r = (int32_t)(a + v + c->c);
        c->v = ((~(a ^ v)) & (a ^ (uint32_t)r) & sign) != 0;
        c->c = (uint32_t)r > mask;
    } else {
        int top = bits / 4 - 1, i;
        int32_t carry = c->c, t;
        r = 0;
        for (i = 0; i < top; i++) {
            t = (int32_t)(((a >> (4 * i)) & 0xF) + ((v >> (4 * i)) & 0xF)) + carry;
            if (t <= 0xF) t -= 6;
            carry = t > 0xF;
            r |= (t & 0xF) << (4 * i);
        }
        t = (int32_t)(((a >> (4 * top)) & 0xF) + ((v >> (4 * top)) & 0xF)) + carry;
        r |= t << (4 * top);
        c->v = ((~(a ^ v)) & (a ^ (uint32_t)r) & sign) != 0;
        if (r <= (int32_t)mask) r -= 6 << (4 * top);
        c->c = r > (int32_t)mask;
    }
    return (uint16_t)((uint32_t)r & mask);
}

static inline void rt_adc8(Cpu *c, uint8_t v) { rt_lda8(c, (uint8_t)rt_adc_core(c, c->a & 0xFF, v, 8)); }
static inline void rt_adc16(Cpu *c, uint16_t v) { rt_lda16(c, rt_adc_core(c, c->a, v, 16)); }
static inline void rt_sbc8(Cpu *c, uint8_t v) { rt_lda8(c, (uint8_t)rt_sbc_core(c, c->a & 0xFF, v, 8)); }
static inline void rt_sbc16(Cpu *c, uint16_t v) { rt_lda16(c, rt_sbc_core(c, c->a, v, 16)); }

/* ---- Shifts, increments and bit tests that return the new value ---- */

#define RT_RMW(W, T, BITS)                                                                  \
    static inline T rt_asl##W(Cpu *c, T v) {                                                \
        T r = (T)(v << 1); c->c = (v >> (BITS - 1)) & 1; rt_nz##W(c, r); return r;         \
    }                                                                                       \
    static inline T rt_lsr##W(Cpu *c, T v) {                                                \
        T r = (T)(v >> 1); c->c = v & 1; rt_nz##W(c, r); return r;                          \
    }                                                                                       \
    static inline T rt_rol##W(Cpu *c, T v) {                                                \
        T r = (T)((v << 1) | c->c); c->c = (v >> (BITS - 1)) & 1; rt_nz##W(c, r); return r; \
    }                                                                                       \
    static inline T rt_ror##W(Cpu *c, T v) {                                                \
        T r = (T)((v >> 1) | ((T)c->c << (BITS - 1))); c->c = v & 1; rt_nz##W(c, r);        \
        return r;                                                                           \
    }                                                                                       \
    static inline T rt_inc##W(Cpu *c, T v) { T r = (T)(v + 1); rt_nz##W(c, r); return r; }   \
    static inline T rt_dec##W(Cpu *c, T v) { T r = (T)(v - 1); rt_nz##W(c, r); return r; }   \
    static inline T rt_tsb##W(Cpu *c, T v) { c->z = ((T)c->a & v) == 0; return (T)(v | c->a); } \
    static inline T rt_trb##W(Cpu *c, T v) { c->z = ((T)c->a & v) == 0; return (T)(v & ~c->a); }

RT_RMW(8, uint8_t, 8)
RT_RMW(16, uint16_t, 16)

/* ---- Block moves: MVN (dir = 1) and MVP (dir = -1) ---- */

static inline void rt_block_move(Cpu *c, uint8_t dst, uint8_t src, int dir) {
    uint16_t mask = c->xf ? 0xFF : 0xFFFF;
    c->db = dst;
    do {
        bus_write8(((uint32_t)dst << 16) | c->y, bus_read8(((uint32_t)src << 16) | c->x));
        c->x = (uint16_t)((c->x + dir) & mask);
        c->y = (uint16_t)((c->y + dir) & mask);
        c->a--;
    } while (c->a != 0xFFFF);
}

#endif /* SNES_RT_H */
