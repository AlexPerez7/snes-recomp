/* Save states. The state is taken and restored only inside rt_resume, the block of the main loop that the profile names in
 * [resume]: there the native stack always has the same shape (the frame of the function that holds the loop), so loading is
 * restoring the globals and the registers of the Cpu and going on. Included by snes_core.inc after the PPU and the contexts.
 *
 * One list of fields (state_io) serves counting, saving and loading, so they cannot drift apart. What is not in it, and why:
 *  - the ROM and the game overlay (they come from the file, and a state must not carry the game);
 *  - the frame buffers and the tables derived at start (the Gaussian kernel, the CRC table): redrawn or recomputed;
 *  - counters of the run (frames, blocks, nmis, irqs, hw_reads...), the trace and the pokes: they belong to the run, not to the game;
 *  - the options (wide, filter...): they are the player's, not the game's;
 *  - the statics of the C of a game (hooks): a game that keeps state there must keep it recoverable from WRAM. */

#define STATE_MAGIC 0x54534E53u /* "SNST" */
#define STATE_FMT 1u

typedef struct {
    int mode; /* 0 count, 1 save, 2 load */
    uint8_t *buf;
    size_t cap, pos;
    int bad;
} StateIO;

static void state_xfer(StateIO *io, void *p, size_t n) {
    if (io->mode == 1) {
        if (io->pos + n > io->cap) {
            size_t cap = io->cap ? io->cap * 2 : (size_t)1 << 20;
            uint8_t *nb;
            while (cap < io->pos + n) cap *= 2;
            nb = (uint8_t *)realloc(io->buf, cap);
            if (!nb) { io->bad = 1; return; }
            io->buf = nb;
            io->cap = cap;
        }
        memcpy(io->buf + io->pos, p, n);
    } else if (io->mode == 2) {
        if (io->pos + n > io->cap) { io->bad = 1; return; }
        memcpy(p, io->buf + io->pos, n);
    }
    io->pos += n;
}

#define SIO(x) state_xfer(io, &(x), sizeof(x))

static uint32_t state_rom_crc(void) {
    static uint32_t crc;
    static int done;
    if (!done) { crc = crc32_update(0, rom, rom_size); done = 1; }
    return crc;
}

/* The fields, in a fixed order. In mode 2 the header has been checked already. */
static void state_io(StateIO *io, Cpu *c) {
    uint32_t magic = STATE_MAGIC, fmt = STATE_FMT, crc = state_rom_crc();
    SIO(magic); SIO(fmt); SIO(crc);
    SIO(*c);
    /* core */
    SIO(wram); SIO(sram); SIO(wram_port); SIO(hw); SIO(fixed_col);
    SIO(vram); SIO(vram_addr); SIO(vram_prefetch); SIO(vram_translate);
    SIO(cgram); SIO(cg_index); SIO(cg_latch); SIO(cg_write_half); SIO(cg_read_half);
    SIO(oam); SIO(oam_byte); SIO(oam_latch);
    SIO(tick_in_frame); SIO(nmi_flag); SIO(nmi_pending); SIO(irq_flag); SIO(irq_fired);
    SIO(joy_read_done); SIO(joy_latched); SIO(joy_cap); SIO(joy_count); SIO(joy_cap2); SIO(joy_count2);
    SIO(hv_h); SIO(hv_v); SIO(hv_latched); SIO(hv_read); SIO(dma_active);
    /* PPU */
    SIO(bg_hofs); SIO(bg_vofs); SIO(bgofs_latch);
    SIO(m7_a); SIO(m7_b); SIO(m7_c); SIO(m7_d); SIO(m7_x0); SIO(m7_y0); SIO(m7_hofs); SIO(m7_vofs); SIO(m7_latch);
    SIO(hdma_ch);
    /* SPC700 and DSP */
    SIO(aram); SIO(spc_in); SIO(spc_out);
    SIO(dsp_reg); SIO(dsp_addr); SIO(voice); SIO(kon_latch); SIO(endx); SIO(dsp_counter); SIO(noise);
    SIO(echo_pos); SIO(echo_len); SIO(echo_hist); SIO(echo_idx);
    SIO(spc_a); SIO(spc_x); SIO(spc_y); SIO(spc_sp); SIO(spc_psw); SIO(spc_f1); SIO(spc_halted); SIO(spc_pc);
    SIO(spc_timer); SIO(dsp_acc); SIO(spc_budget);
}

/* Frames without a switch between execution contexts that make a leftover suspended context harmless. */
#define STATE_QUIET_FRAMES 120

/* Why the execution is not where a state can be taken or loaded, or NULL if it is. The main context must be running with no
 * interrupt in progress. Other contexts cannot be saved (their native stacks are not part of a state), so they must be out
 * of play: ALttP swaps stacks between NMI and IRQ in its intro and leaves a suspended context behind that it never resumes
 * in the game, so what is required is that no switch has happened for a while. */
static const char *state_point_why(void) {
    int j;
    if (cur_ctx != 0) return "another execution context is running";
    if (in_nmi || ctxs[0].depth) return "an interrupt is running";
    for (j = 1; j < MAX_CTX; j++) {
        if (ctxs[j].used && frames - ctx_last_switch < STATE_QUIET_FRAMES) return "the game is switching execution contexts";
    }
    return NULL;
}

/* Result of the last request, for the frontend: 0 none yet, 1 done, -1 rejected (state_msg says why). */
static int state_req;                /* 0 none, 1 save, 2 load */
static uint8_t *state_data;          /* the state to load, or the one just saved (malloc) */
static size_t state_len;
static int state_result;
static const char *state_msg = "";

/* Frees the state held in state_data. */
static void state_release(void) {
    free(state_data);
    state_data = NULL;
    state_len = 0;
}

static void state_do_save(Cpu *c) {
    StateIO io;
    memset(&io, 0, sizeof io);
    io.mode = 1;
    state_io(&io, c);
    if (io.bad) {
        free(io.buf);
        state_result = -1;
        state_msg = "out of memory";
        return;
    }
    state_release();
    state_data = io.buf;
    state_len = io.pos;
    state_result = 1;
    state_msg = "saved";
}

static void state_do_load(Cpu *c) {
    StateIO io;
    uint32_t head[3], want[3] = {STATE_MAGIC, STATE_FMT, 0};
    Cpu saved;
    size_t total;
    memset(&io, 0, sizeof io);
    io.mode = 0;
    state_io(&io, c);
    total = io.pos;
    want[2] = state_rom_crc();
    state_result = -1;
    if (!state_data || state_len != total || memcmp(state_data, want, 8) != 0) {
        state_msg = "not a state of this version";
        return;
    }
    memcpy(head, state_data, sizeof head);
    if (head[2] != want[2]) { state_msg = "state of another game"; return; }
    memcpy(&saved, state_data + sizeof head, sizeof saved);
    if (saved.s != c->s) { state_msg = "state taken at another stack depth"; return; }
    io.mode = 2;
    io.buf = state_data;
    io.cap = state_len;
    io.pos = 0;
    io.bad = 0;
    state_io(&io, c);
    /* The picture of the frame in progress is not part of the state: redraw it from the first row. */
    ppu_hold = 0;
    ppu_inited = 0;
    ppu_row = 0;
    ppu_update_next();
    state_result = 1;
    state_msg = "loaded";
}

/* Called by rt_resume. */
static void state_service(Cpu *c) {
    const char *why = state_point_why();
    int req = state_req;
    if (why) { /* not here: the request waits for the next pass through the resume point */
        state_msg = why;
        return;
    }
    state_req = 0;
    state_result = 0;
    if (req == 1) state_do_save(c);
    else if (req == 2) state_do_load(c);
}
