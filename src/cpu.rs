use super::memory::Memory;
use super::mmu;

/// The register F holds flag information that are set by ALU
/// operations. Conditional operations check these flags afterwards.
enum Flag {
    /// Zero flag is set when operations result in zero values
    Z = 0b1000_0000,
    /// Negative flag is set when a subtraction operation is performed
    N = 0b0100_0000,
    /// Half-carry flag is set when an operation creates a carry bit from bit 3 to 4.
    H = 0b0010_0000,
    /// Carry flag is set when an operation creates a carry bit from bit 7.
    C = 0b0001_0000,
}

/// Represents all the registers in use by the Gameboy CPU.
/// Consists of 16-bit register pairs that can be accessed as 8-bit
/// high and low registers and as combined 16-bit values
/// Paired as follows:
/// - AF
/// - BC
/// - DE
/// - HL
///
/// Also contains two other 16-bit registers:
/// - PC (Program Counter)
/// - SP (Stack Pointer)
#[derive(Clone, Default)]
struct Registers {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

impl Registers {
    /// Initializes the state of the Registers of the CPU
    /// Simulates the state of the CPU post-BIOS and right before running
    /// user code
    fn power_on() -> Self {
        // Default to all zeros
        let mut reg = Self::default();

        // Simulate BIOS procedure that initializes values
        reg.a = 0x01;
        reg.f = 0xB0;
        reg.b = 0x00;
        reg.c = 0x13;
        reg.d = 0x00;
        reg.e = 0xD8;
        reg.h = 0x01;
        reg.l = 0x4D;
        reg.sp = 0xFFFE;

        // Start at memory location 0x0100 after running the BIOS procedure
        // This is where actual ROM game code begins
        reg.pc = 0x0100;
        reg
    }

    /// Returns a 16-bit value where
    /// A is the hi 8-bits and F is the lo 8-bits
    fn get_af(&self) -> u16 {
        ((self.a as u16) << 8) | (self.f as u16)
    }

    /// Returns a 16-bit value where
    /// B is the hi 8-bits and C is the lo 8-bits
    fn get_bc(&self) -> u16 {
        ((self.b as u16) << 8) | (self.c as u16)
    }

    /// Returns a 16-bit value where
    /// D is the hi 8-bits and E is the lo 8-bits
    fn get_de(&self) -> u16 {
        ((self.d as u16) << 8) | (self.e as u16)
    }

    /// Returns a 16-bit value where
    /// H is the hi 8-bits and L is the lo 8-bits
    fn get_hl(&self) -> u16 {
        ((self.h as u16) << 8) | (self.l as u16)
    }

    /// Sets a 16-bit value where
    /// A is the hi 8-bits and F is the lo 8-bits
    fn set_af(&mut self, val: u16) {
        self.a = (val >> 8) as u8;
        self.f = (val & 0xFF) as u8;
    }

    /// Sets a 16-bit value where
    /// B is the hi 8-bits and C is the lo 8-bits
    fn set_bc(&mut self, val: u16) {
        self.b = (val >> 8) as u8;
        self.c = (val & 0xFF) as u8;
    }

    /// Sets a 16-bit value where
    /// D is the hi 8-bits and E is the lo 8-bits
    fn set_de(&mut self, val: u16) {
        self.d = (val >> 8) as u8;
        self.e = (val & 0xFF) as u8;
    }

    /// Sets a 16-bit value where
    /// H is the hi 8-bits and L is the lo 8-bits
    fn set_hl(&mut self, val: u16) {
        self.h = (val >> 8) as u8;
        self.l = (val & 0xFF) as u8;
    }

    fn set_flag(&mut self, f: Flag, v: bool) {
        match v {
            true => self.f |= f as u8,
            false => self.f &= !(f as u8),
        };
    }

    fn get_flag(&mut self, f: Flag) -> bool {
        (self.f & (f as u8)) != 0
    }
}

/// Tables of opcode cycle counts.
/// Skipped when running rustfmt
#[rustfmt::skip]
const OPCODE_TABLE: [usize; 256] = [
//  0  1  2  3  4  5  6  7  8  9  A  B  C  D  E  F
    4,12, 8, 8, 4, 4, 8, 4,20, 8, 8, 8, 4, 4, 8, 4, // 0
    4,12, 8, 8, 4, 4, 8, 4,12, 8, 8, 8, 4, 4, 8, 4, // 1
    8,12, 8, 8, 4, 4, 8, 4, 8, 8, 8, 8, 4, 4, 8, 4, // 2
    8,12, 8, 8,12,12,12, 4, 8, 8, 8, 8, 4, 4, 8, 4, // 3
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4, // 4
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4, // 5
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4, // 6
    8, 8, 8, 8, 8, 8, 4, 8, 4, 4, 4, 4, 4, 4, 8, 4, // 7
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4, // 8
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4, // 9
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4, // A
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4, // B
    8,12,12,16,12,16, 8,16, 8,16,12, 4,12,24, 8,16, // C
    8,12,12, 0,12,16, 8,16, 8,16,12, 0,12, 0, 8,16, // D
   12,12, 8, 0, 0,16, 8,16,16, 4,16, 0, 0, 0, 8,16, // E
   12,12, 8, 4, 0,16, 8,16,12, 8,16, 4, 0, 0, 8,16, // F
];

/// Tables of opcode cycle counts for extended opcodes.
/// Skipped when running rustfmt
#[rustfmt::skip]
const OPCODE_CB_TABLE: [usize; 256] = [
//  0  1  2  3  4  5  6  7  8  9  A  B  C  D  E  F
    8, 8, 8, 8, 8, 8,16, 8, 8, 8, 8, 8, 8, 8,16, 8, // 0
    8, 8, 8, 8, 8, 8,16, 8, 8, 8, 8, 8, 8, 8,16, 8, // 1
    8, 8, 8, 8, 8, 8,16, 8, 8, 8, 8, 8, 8, 8,16, 8, // 2
    8, 8, 8, 8, 8, 8,16, 8, 8, 8, 8, 8, 8, 8,16, 8, // 3
    8, 8, 8, 8, 8, 8,16, 8, 8, 8, 8, 8, 8, 8,16, 8, // 4
    8, 8, 8, 8, 8, 8,16, 8, 8, 8, 8, 8, 8, 8,16, 8, // 5
    8, 8, 8, 8, 8, 8,16, 8, 8, 8, 8, 8, 8, 8,16, 8, // 6
    8, 8, 8, 8, 8, 8,16, 8, 8, 8, 8, 8, 8, 8,16, 8, // 7
    8, 8, 8, 8, 8, 8,16, 8, 8, 8, 8, 8, 8, 8,16, 8, // 8
    8, 8, 8, 8, 8, 8,16, 8, 8, 8, 8, 8, 8, 8,16, 8, // 9
    8, 8, 8, 8, 8, 8,16, 8, 8, 8, 8, 8, 8, 8,16, 8, // A
    8, 8, 8, 8, 8, 8,16, 8, 8, 8, 8, 8, 8, 8,16, 8, // B
    8, 8, 8, 8, 8, 8,16, 8, 8, 8, 8, 8, 8, 8,16, 8, // C
    8, 8, 8, 8, 8, 8,16, 8, 8, 8, 8, 8, 8, 8,16, 8, // D
    8, 8, 8, 8, 8, 8,16, 8, 8, 8, 8, 8, 8, 8,16, 8, // E
    8, 8, 8, 8, 8, 8,16, 8, 8, 8, 8, 8, 8, 8,16, 8, // F
];

const OPCODE_STRINGS: [&str; 256] = [
    "NOP",
    "LD BC,d16",
    "LD (BC),A",
    "INC BC",
    "INC B",
    "DEC B",
    "LD B,d8",
    "RLCA",
    "LD (a16),SP",
    "ADD HL,BC",
    "LD A,(BC)",
    "DEC BC",
    "INC C",
    "DEC C",
    "LD C,d8",
    "RRCA",
    "STOP 0",
    "LD DE,d16",
    "LD (DE),A",
    "INC DE",
    "INC D",
    "DEC D",
    "LD D,d8",
    "RLA",
    "JR r8",
    "ADD HL,DE",
    "LD A,(DE)",
    "DEC DE",
    "INC E",
    "DEC E",
    "LD E,d8",
    "RRA",
    "JR NZ,r8",
    "LD HL,d16",
    "LD (HL+),A",
    "INC HL",
    "INC H",
    "DEC H",
    "LD H,d8",
    "DAA",
    "JR Z,r8",
    "ADD HL,HL",
    "LD A,(HL+)",
    "DEC HL",
    "INC L",
    "DEC L",
    "LD L,d8",
    "CPL",
    "JR NC,r8",
    "LD SP,d16",
    "LD (HL-),A",
    "INC SP",
    "INC (HL)",
    "DEC (HL)",
    "LD (HL),d8",
    "SCF",
    "JR C,r8",
    "ADD HL,SP",
    "LD A,(HL-)",
    "DEC SP",
    "INC A",
    "DEC A",
    "LD A,d8",
    "CCF",
    "LD B,B",
    "LD B,C",
    "LD B,D",
    "LD B,E",
    "LD B,H",
    "LD B,L",
    "LD B,(HL)",
    "LD B,A",
    "LD C,B",
    "LD C,C",
    "LD C,D",
    "LD C,E",
    "LD C,H",
    "LD C,L",
    "LD C,(HL)",
    "LD C,A",
    "LD D,B",
    "LD D,C",
    "LD D,D",
    "LD D,E",
    "LD D,H",
    "LD D,L",
    "LD D,(HL)",
    "LD D,A",
    "LD E,B",
    "LD E,C",
    "LD E,D",
    "LD E,E",
    "LD E,H",
    "LD E,L",
    "LD E,(HL)",
    "LD E,A",
    "LD H,B",
    "LD H,C",
    "LD H,D",
    "LD H,E",
    "LD H,H",
    "LD H,L",
    "LD H,(HL)",
    "LD H,A",
    "LD L,B",
    "LD L,C",
    "LD L,D",
    "LD L,E",
    "LD L,H",
    "LD L,L",
    "LD L,(HL)",
    "LD L,A",
    "LD (HL),B",
    "LD (HL),C",
    "LD (HL),D",
    "LD (HL),E",
    "LD (HL),H",
    "LD (HL),L",
    "HALT",
    "LD (HL),A",
    "LD A,B",
    "LD A,C",
    "LD A,D",
    "LD A,E",
    "LD A,H",
    "LD A,L",
    "LD A,(HL)",
    "LD A,A",
    "ADD A,B",
    "ADD A,C",
    "ADD A,D",
    "ADD A,E",
    "ADD A,H",
    "ADD A,L",
    "ADD A,(HL)",
    "ADD A,A",
    "ADC A,B",
    "ADC A,C",
    "ADC A,D",
    "ADC A,E",
    "ADC A,H",
    "ADC A,L",
    "ADC A,(HL)",
    "ADC A,A",
    "SUB A,B",
    "SUB A,C",
    "SUB A,D",
    "SUB A,E",
    "SUB A,H",
    "SUB A,L",
    "SUB A,(HL)",
    "SUB A,A",
    "SBC A,B",
    "SBC A,C",
    "SBC A,D",
    "SBC A,E",
    "SBC A,H",
    "SBC A,L",
    "SBC A,(HL)",
    "SBC A,A",
    "AND B",
    "AND C",
    "AND D",
    "AND E",
    "AND H",
    "AND L",
    "AND (HL)",
    "AND A",
    "XOR B",
    "XOR C",
    "XOR D",
    "XOR E",
    "XOR H",
    "XOR L",
    "XOR (HL)",
    "XOR A",
    "OR B",
    "OR C",
    "OR D",
    "OR E",
    "OR H",
    "OR L",
    "OR (HL)",
    "OR A",
    "CP B",
    "CP C",
    "CP D",
    "CP E",
    "CP H",
    "CP L",
    "CP (HL)",
    "CP A",
    "RET NZ",
    "POP BC",
    "JP NZ,a16",
    "JP a16",
    "CALL NZ,a16",
    "PUSH BC",
    "ADD A,d8",
    "RST 00H",
    "RET Z",
    "RET",
    "JP Z,a16",
    "CB ",
    "CALL Z,a16",
    "CALL a16",
    "ADC A,d8",
    "RST 08H",
    "RET NC",
    "POP DE",
    "JP NC,a16",
    "NULL",
    "CALL NC,a16",
    "PUSH DE",
    "SUB d8",
    "RST 10H",
    "RET C",
    "RETI",
    "JP C,a16",
    "NULL",
    "CALL C,a16",
    "NULL",
    "SBC A,d8",
    "RST 18H",
    "LDH (a8),A",
    "POP HL",
    "LD (C),A",
    "NULL",
    "NULL",
    "PUSH HL",
    "AND d8",
    "RST 20H",
    "ADD SP,r8",
    "JP (HL)",
    "JP (a16),A",
    "NULL",
    "NULL",
    "NULL",
    "XOR d8",
    "RST 28H",
    "LDH A,(a8)",
    "POP AF",
    "LD A,(C)",
    "DI",
    "NULL",
    "PUSH AF",
    "OR d8",
    "RST 30H",
    "LD HL,SP+r8",
    "LD SP,HL",
    "JP A,(a16)",
    "EI",
    "NULL",
    "NULL",
    "CP d8",
    "RST 38H",
];

/// The CPU contains Register state and is responsible for
/// decoding each opcode at the current PC and updating
/// the Registers and MMU when appropriate.
pub struct Cpu {
    reg: Registers,
    ime: bool,
    halted: bool,
}

impl Cpu {
    /// Initializes CPU internal state and returns a handle to the
    /// initialized Cpu struct.
    pub fn power_on() -> Self {
        Cpu {
            reg: Registers::power_on(),
            ime: false,
            halted: false,
        }
    }

    /// Fetches a single instruction opcode, decodes the opcode to the
    /// appropriate function, and executes the functionality.
    /// Returns the number of cycles executed.
    pub fn tick(&mut self, mmu: &mut mmu::Mmu) -> usize {
        let old_pc = self.reg.pc;
        let opcode = self.imm(mmu);
        trace!(
            "0x{:04X}: 0x{:02X} {}",
            old_pc,
            opcode,
            OPCODE_STRINGS[opcode as usize]
        );
        // Use more cycles when following conditional branches,
        // set when conditionals are met.
        let mut cond_cycles: usize = 0;
        match opcode {
            // NOP
            0x00 => (),

            // HALT
            0x76 => self.halted = true,

            // STOP
            0x10 => unimplemented!("STOP not implemented"),

            // IME
            0xF3 => self.ime = false,
            0xFB => self.ime = true,

            // LD r8,d8
            0x06 => self.reg.b = self.imm(mmu),
            0x0E => self.reg.c = self.imm(mmu),
            0x16 => self.reg.d = self.imm(mmu),
            0x1E => self.reg.e = self.imm(mmu),
            0x26 => self.reg.h = self.imm(mmu),
            0x2E => self.reg.l = self.imm(mmu),
            0x36 => {
                let v = self.imm(mmu);
                mmu.write_byte(self.reg.get_hl(), v);
            }
            0x3E => self.reg.a = self.imm(mmu),

            // LD (r16),A
            0x02 => mmu.write_byte(self.reg.get_bc(), self.reg.a),
            0x12 => mmu.write_byte(self.reg.get_de(), self.reg.a),

            // LD A,(r16)
            0x0a => self.reg.a = mmu.read_byte(self.reg.get_bc()),
            0x1a => self.reg.a = mmu.read_byte(self.reg.get_de()),

            // LD (HL+),A
            0x22 => {
                let v = self.reg.get_hl();
                mmu.write_byte(v, self.reg.a);
                self.reg.set_hl(v + 1);
            }

            // LD (HL-),A
            0x32 => {
                let v = self.reg.get_hl();
                mmu.write_byte(v, self.reg.a);
                self.reg.set_hl(v - 1);
            }

            // LD A,(HL+)
            0x2a => {
                let v = self.reg.get_hl();
                self.reg.a = mmu.read_byte(v);
                self.reg.set_hl(v + 1);
            }

            // LD A,(HL-)
            0x3a => {
                let v = self.reg.get_hl();
                self.reg.a = mmu.read_byte(v);
                self.reg.set_hl(v - 1);
            }

            // LD r8,r8
            0x40 => (),
            0x41 => self.reg.b = self.reg.c,
            0x42 => self.reg.b = self.reg.d,
            0x43 => self.reg.b = self.reg.e,
            0x44 => self.reg.b = self.reg.h,
            0x45 => self.reg.b = self.reg.l,
            0x46 => self.reg.b = mmu.read_byte(self.reg.get_hl()),
            0x47 => self.reg.b = self.reg.a,
            0x48 => self.reg.c = self.reg.b,
            0x49 => (),
            0x4A => self.reg.c = self.reg.d,
            0x4B => self.reg.c = self.reg.e,
            0x4C => self.reg.c = self.reg.h,
            0x4D => self.reg.c = self.reg.l,
            0x4E => self.reg.c = mmu.read_byte(self.reg.get_hl()),
            0x4F => self.reg.c = self.reg.a,
            0x50 => self.reg.d = self.reg.b,
            0x51 => self.reg.d = self.reg.c,
            0x52 => (),
            0x53 => self.reg.d = self.reg.e,
            0x54 => self.reg.d = self.reg.h,
            0x55 => self.reg.d = self.reg.l,
            0x56 => self.reg.d = mmu.read_byte(self.reg.get_hl()),
            0x57 => self.reg.d = self.reg.a,
            0x58 => self.reg.e = self.reg.b,
            0x59 => self.reg.e = self.reg.c,
            0x5A => self.reg.e = self.reg.d,
            0x5B => (),
            0x5C => self.reg.e = self.reg.h,
            0x5D => self.reg.e = self.reg.l,
            0x5E => self.reg.e = mmu.read_byte(self.reg.get_hl()),
            0x5F => self.reg.e = self.reg.a,
            0x60 => self.reg.h = self.reg.b,
            0x61 => self.reg.h = self.reg.c,
            0x62 => self.reg.h = self.reg.d,
            0x63 => self.reg.h = self.reg.e,
            0x64 => (),
            0x65 => self.reg.h = self.reg.l,
            0x66 => self.reg.h = mmu.read_byte(self.reg.get_hl()),
            0x67 => self.reg.h = self.reg.a,
            0x68 => self.reg.l = self.reg.b,
            0x69 => self.reg.l = self.reg.c,
            0x6A => self.reg.l = self.reg.d,
            0x6B => self.reg.l = self.reg.e,
            0x6C => self.reg.l = self.reg.h,
            0x6D => (),
            0x6E => self.reg.l = mmu.read_byte(self.reg.get_hl()),
            0x6F => self.reg.l = self.reg.a,
            0x70 => mmu.write_byte(self.reg.get_hl(), self.reg.b),
            0x71 => mmu.write_byte(self.reg.get_hl(), self.reg.c),
            0x72 => mmu.write_byte(self.reg.get_hl(), self.reg.d),
            0x73 => mmu.write_byte(self.reg.get_hl(), self.reg.e),
            0x74 => mmu.write_byte(self.reg.get_hl(), self.reg.h),
            0x75 => mmu.write_byte(self.reg.get_hl(), self.reg.l),
            0x76 => (), // TODO: HALT
            0x77 => mmu.write_byte(self.reg.get_hl(), self.reg.a),
            0x78 => self.reg.a = self.reg.b,
            0x79 => self.reg.a = self.reg.c,
            0x7A => self.reg.a = self.reg.d,
            0x7B => self.reg.a = self.reg.e,
            0x7C => self.reg.a = self.reg.h,
            0x7D => self.reg.a = self.reg.l,
            0x7E => self.reg.a = mmu.read_byte(self.reg.get_hl()),
            0x7F => (),

            // LD r16,d16
            0x01 => {
                let v = self.imm_word(mmu);
                self.reg.set_bc(v);
            }
            0x11 => {
                let v = self.imm_word(mmu);
                self.reg.set_de(v);
            }
            0x21 => {
                let v = self.imm_word(mmu);
                self.reg.set_hl(v);
            }
            0x31 => {
                let v = self.imm_word(mmu);
                self.reg.sp = v;
            }

            // LD (a16),SP
            0x08 => {
                let v = self.imm_word(mmu);
                mmu.write_word(v, self.reg.sp);
            }

            // LD SP,HL
            0xF9 => self.reg.sp = self.reg.get_hl(),

            // ADD A,r8
            0x80 => self.add(self.reg.b),
            0x81 => self.add(self.reg.c),
            0x82 => self.add(self.reg.d),
            0x83 => self.add(self.reg.e),
            0x84 => self.add(self.reg.h),
            0x85 => self.add(self.reg.l),
            0x86 => self.add(mmu.read_byte(self.reg.get_hl())),
            0x87 => self.add(self.reg.a),

            // ADD A,d8
            0xC6 => {
                let v = self.imm(mmu);
                self.add(v);
            }

            // ADC A,r8
            0x88 => self.adc(self.reg.b),
            0x89 => self.adc(self.reg.c),
            0x8A => self.adc(self.reg.d),
            0x8B => self.adc(self.reg.e),
            0x8C => self.adc(self.reg.h),
            0x8D => self.adc(self.reg.l),
            0x8E => self.adc(mmu.read_byte(self.reg.get_hl())),
            0x8F => self.adc(self.reg.a),

            // ADC A,d8
            0xCE => {
                let v = self.imm(mmu);
                self.adc(v);
            }

            // ADD SP,r8
            0xE8 => self.add_sp(mmu),

            // ADD HL,r16
            0x09 => self.add_hl(self.reg.get_bc()),
            0x19 => self.add_hl(self.reg.get_de()),
            0x29 => self.add_hl(self.reg.get_hl()),
            0x39 => self.add_hl(self.reg.sp),

            // SUB r8
            0x90 => self.sub(self.reg.b),
            0x91 => self.sub(self.reg.c),
            0x92 => self.sub(self.reg.d),
            0x93 => self.sub(self.reg.e),
            0x94 => self.sub(self.reg.h),
            0x95 => self.sub(self.reg.l),
            0x96 => self.sub(mmu.read_byte(self.reg.get_hl())),
            0x97 => self.sub(self.reg.a),

            // SUB d8
            0xD6 => {
                let v = self.imm(mmu);
                self.sub(v);
            }

            // SBC r8,r8
            0x98 => self.sbc(self.reg.b),
            0x99 => self.sbc(self.reg.c),
            0x9A => self.sbc(self.reg.d),
            0x9B => self.sbc(self.reg.e),
            0x9C => self.sbc(self.reg.h),
            0x9D => self.sbc(self.reg.l),
            0x9E => self.sbc(mmu.read_byte(self.reg.get_hl())),
            0x9F => self.sbc(self.reg.a),

            // SBC d8
            0xDE => {
                let v = self.imm(mmu);
                self.sbc(v);
            }

            // AND r8
            0xA0 => self.and(self.reg.b),
            0xA1 => self.and(self.reg.c),
            0xA2 => self.and(self.reg.d),
            0xA3 => self.and(self.reg.e),
            0xA4 => self.and(self.reg.h),
            0xA5 => self.and(self.reg.l),
            0xA6 => self.and(mmu.read_byte(self.reg.get_hl())),
            0xA7 => self.and(self.reg.a),

            // AND d8
            0xE6 => {
                let v = self.imm(mmu);
                self.and(v);
            }

            // XOR r8
            0xA8 => self.xor(self.reg.b),
            0xA9 => self.xor(self.reg.c),
            0xAA => self.xor(self.reg.d),
            0xAB => self.xor(self.reg.e),
            0xAC => self.xor(self.reg.h),
            0xAD => self.xor(self.reg.l),
            0xAE => self.xor(mmu.read_byte(self.reg.get_hl())),
            0xAF => self.xor(self.reg.a),

            // XOR d8
            0xEE => {
                let v = self.imm(mmu);
                self.xor(v);
            }

            // OR r8
            0xB0 => self.or(self.reg.b),
            0xB1 => self.or(self.reg.c),
            0xB2 => self.or(self.reg.d),
            0xB3 => self.or(self.reg.e),
            0xB4 => self.or(self.reg.h),
            0xB5 => self.or(self.reg.l),
            0xB6 => self.or(mmu.read_byte(self.reg.get_hl())),
            0xB7 => self.or(self.reg.a),

            // OR d8
            0xF6 => {
                let v = self.imm(mmu);
                self.or(v);
            }

            // CP r8
            0xB8 => self.cp(self.reg.b),
            0xB9 => self.cp(self.reg.c),
            0xBA => self.cp(self.reg.d),
            0xBB => self.cp(self.reg.e),
            0xBC => self.cp(self.reg.h),
            0xBD => self.cp(self.reg.l),
            0xBE => self.cp(mmu.read_byte(self.reg.get_hl())),
            0xBF => self.cp(self.reg.a),

            // CP d8
            0xFE => {
                let v = self.imm(mmu);
                self.cp(v);
            }

            // INC r8
            0x04 => self.reg.b = self.inc(self.reg.b),
            0x0C => self.reg.c = self.inc(self.reg.c),
            0x14 => self.reg.d = self.inc(self.reg.d),
            0x1C => self.reg.e = self.inc(self.reg.e),
            0x24 => self.reg.h = self.inc(self.reg.h),
            0x2C => self.reg.l = self.inc(self.reg.l),
            0x34 => {
                let v = self.inc(mmu.read_byte(self.reg.get_hl()));
                mmu.write_byte(self.reg.get_hl(), v);
            }
            0x3C => self.reg.a = self.inc(self.reg.a),

            // DEC r8
            0x05 => self.reg.b = self.dec(self.reg.b),
            0x0D => self.reg.c = self.dec(self.reg.c),
            0x15 => self.reg.d = self.dec(self.reg.d),
            0x1D => self.reg.e = self.dec(self.reg.e),
            0x25 => self.reg.h = self.dec(self.reg.h),
            0x2D => self.reg.l = self.dec(self.reg.l),
            0x35 => {
                let v = self.dec(mmu.read_byte(self.reg.get_hl()));
                mmu.write_byte(self.reg.get_hl(), v);
            }
            0x3D => self.reg.a = self.dec(self.reg.a),

            // INC r16
            0x03 => self.reg.set_bc(self.reg.get_bc().wrapping_add(1)),
            0x13 => self.reg.set_de(self.reg.get_de().wrapping_add(1)),
            0x23 => self.reg.set_hl(self.reg.get_hl().wrapping_add(1)),
            0x33 => self.reg.sp = self.reg.sp.wrapping_add(1),

            // DEC r16
            0x0B => self.reg.set_bc(self.reg.get_bc().wrapping_sub(1)),
            0x1B => self.reg.set_de(self.reg.get_de().wrapping_sub(1)),
            0x2B => self.reg.set_hl(self.reg.get_hl().wrapping_sub(1)),
            0x3B => self.reg.sp = self.reg.sp.wrapping_sub(1),

            // POP r16
            0xC1 => {
                let v = self.stack_pop(mmu);
                self.reg.set_bc(v);
            }
            0xD1 => {
                let v = self.stack_pop(mmu);
                self.reg.set_de(v);
            }
            0xE1 => {
                let v = self.stack_pop(mmu);
                self.reg.set_hl(v);
            }
            0xF1 => {
                let v = self.stack_pop(mmu);
                self.reg.set_af(v);
            }

            // PUSH r16
            0xC5 => {
                let v = self.reg.get_bc();
                self.stack_push(mmu, v);
            }
            0xD5 => {
                let v = self.reg.get_de();
                self.stack_push(mmu, v);
            }
            0xE5 => {
                let v = self.reg.get_hl();
                self.stack_push(mmu, v);
            }
            0xF5 => {
                let v = self.reg.get_af();
                self.stack_push(mmu, v);
            }

            // JP
            0xC3 => {
                let a = self.imm_word(mmu);
                self.reg.pc = a;
            }
            0xE9 => {
                let a = self.reg.get_hl();
                self.reg.pc = a;
            }
            0xC2 => {
                let a = self.imm_word(mmu);
                if !self.reg.get_flag(Flag::Z) {
                    self.reg.pc = a;
                    cond_cycles = 4;
                }
            }
            0xD2 => {
                let a = self.imm_word(mmu);
                if !self.reg.get_flag(Flag::C) {
                    self.reg.pc = a;
                    cond_cycles = 4;
                }
            }
            0xCA => {
                let a = self.imm_word(mmu);
                if self.reg.get_flag(Flag::Z) {
                    self.reg.pc = a;
                    cond_cycles = 4;
                }
            }
            0xDA => {
                let a = self.imm_word(mmu);
                if self.reg.get_flag(Flag::C) {
                    self.reg.pc = a;
                    cond_cycles = 4;
                }
            }

            // JR
            0x18 => {
                let a = self.imm(mmu) as i8;
                self.reg.pc = self.reg.pc.wrapping_add(a as u16);
            }
            0x20 => {
                let a = self.imm(mmu) as i8;
                if !self.reg.get_flag(Flag::Z) {
                    self.reg.pc = self.reg.pc.wrapping_add(a as u16);
                    cond_cycles = 4;
                }
            }
            0x30 => {
                let a = self.imm(mmu) as i8;
                if !self.reg.get_flag(Flag::C) {
                    self.reg.pc = self.reg.pc.wrapping_add(a as u16);
                    cond_cycles = 4;
                }
            }
            0x28 => {
                let a = self.imm(mmu) as i8;
                if self.reg.get_flag(Flag::Z) {
                    self.reg.pc = self.reg.pc.wrapping_add(a as u16);
                    cond_cycles = 4;
                }
            }
            0x38 => {
                let a = self.imm(mmu) as i8;
                if self.reg.get_flag(Flag::C) {
                    self.reg.pc = self.reg.pc.wrapping_add(a as u16);
                    cond_cycles = 4;
                }
            }

            // CALL
            0xCD => {
                let a = self.imm_word(mmu);
                self.stack_push(mmu, self.reg.pc);
                self.reg.pc = a;
            }
            0xC4 => {
                let a = self.imm_word(mmu);
                if !self.reg.get_flag(Flag::Z) {
                    self.stack_push(mmu, self.reg.pc);
                    self.reg.pc = a;
                    cond_cycles = 12;
                }
            }
            0xCC => {
                let a = self.imm_word(mmu);
                if self.reg.get_flag(Flag::Z) {
                    self.stack_push(mmu, self.reg.pc);
                    self.reg.pc = a;
                    cond_cycles = 12;
                }
            }
            0xD4 => {
                let a = self.imm_word(mmu);
                if !self.reg.get_flag(Flag::C) {
                    self.stack_push(mmu, self.reg.pc);
                    self.reg.pc = a;
                    cond_cycles = 12;
                }
            }
            0xDC => {
                let a = self.imm_word(mmu);
                if self.reg.get_flag(Flag::C) {
                    self.stack_push(mmu, self.reg.pc);
                    self.reg.pc = a;
                    cond_cycles = 12;
                }
            }

            // RET
            0xC9 => {
                let a = self.stack_pop(mmu);
                self.reg.pc = a;
            }
            0xC0 => {
                if !self.reg.get_flag(Flag::Z) {
                    let a = self.stack_pop(mmu);
                    self.reg.pc = a;
                    cond_cycles = 12;
                }
            }
            0xC8 => {
                if self.reg.get_flag(Flag::Z) {
                    let a = self.stack_pop(mmu);
                    self.reg.pc = a;
                    cond_cycles = 12;
                }
            }
            0xD0 => {
                if !self.reg.get_flag(Flag::C) {
                    let a = self.stack_pop(mmu);
                    self.reg.pc = a;
                    cond_cycles = 12;
                }
            }
            0xD8 => {
                if self.reg.get_flag(Flag::C) {
                    let a = self.stack_pop(mmu);
                    self.reg.pc = a;
                    cond_cycles = 12;
                }
            }
            0xD9 => {
                let a = self.stack_pop(mmu);
                self.reg.pc = a;
                self.ime = true;
            }

            // RST
            0xC7 => {
                self.stack_push(mmu, self.reg.pc);
                self.reg.pc = 0x00;
            }
            0xCF => {
                self.stack_push(mmu, self.reg.pc);
                self.reg.pc = 0x08;
            }
            0xD7 => {
                self.stack_push(mmu, self.reg.pc);
                self.reg.pc = 0x10;
            }
            0xDF => {
                self.stack_push(mmu, self.reg.pc);
                self.reg.pc = 0x18;
            }
            0xE7 => {
                self.stack_push(mmu, self.reg.pc);
                self.reg.pc = 0x20;
            }
            0xEF => {
                self.stack_push(mmu, self.reg.pc);
                self.reg.pc = 0x28;
            }
            0xF7 => {
                self.stack_push(mmu, self.reg.pc);
                self.reg.pc = 0x30;
            }
            0xFF => {
                self.stack_push(mmu, self.reg.pc);
                self.reg.pc = 0x38;
            }

            // CB Prefix
            0xCB => {
                let cb_opcode = self.imm(mmu);
                match cb_opcode {
                    _ => panic!("Unsupported or unimplemented opcode 0xCB {:X}", opcode),
                }
            }
            _ => panic!("Unsupported or unimplemented opcode 0x{:X}", opcode),
        };
        OPCODE_TABLE[opcode as usize] + cond_cycles
    }

    /// Reads and returns the value at the current PC location
    /// Increments the PC after reading
    fn imm(&mut self, mmu: &mut mmu::Mmu) -> u8 {
        let v = mmu.read_byte(self.reg.pc);
        self.reg.pc += 1;
        v
    }

    /// Reads and returns the word at the current PC location
    /// Value is little endian representation
    /// Increments PC to after the word
    fn imm_word(&mut self, mmu: &mut mmu::Mmu) -> u16 {
        let lo = self.imm(mmu);
        let hi = self.imm(mmu);
        ((hi as u16) << 8) | (lo as u16)
    }

    fn stack_push(&mut self, mmu: &mut mmu::Mmu, v: u16) {
        self.reg.sp -= 2;
        mmu.write_word(self.reg.sp, v);
    }

    fn stack_pop(&mut self, mmu: &mut mmu::Mmu) -> u16 {
        let v = mmu.read_word(self.reg.sp);
        self.reg.sp += 2;
        v
    }

    /// Adds the given register value `r` to the `A` register.
    /// Flags:
    ///
    /// - Z: Set to 1 if resulting value is 0, set to 0 otherwise
    /// - N: Set to 0
    /// - H: Set to 1 if bit 3 has a carry, 0 otherwise
    /// - C: Set to 1 if bit 7 has a carry, 0 otherwise
    fn add(&mut self, r: u8) {
        let v = self.reg.a.wrapping_add(r);
        // Evaluate flags
        self.reg.set_flag(Flag::Z, v == 0);
        self.reg.set_flag(Flag::N, false);
        self.reg
            .set_flag(Flag::H, (self.reg.a & 0x0F) + (r & 0x0F) > 0x0F);
        self.reg
            .set_flag(Flag::C, u16::from(self.reg.a) + u16::from(r) > 0xFF);
        self.reg.a = v;
    }

    /// Adds the given register value `r` and carry flag to the `A` register.
    /// Flags:
    ///
    /// - Z: Set to 1 if resulting value is 0, set to 0 otherwise
    /// - N: Set to 0
    /// - H: Set to 1 if bit 3 has a carry, 0 otherwise
    /// - C: Set to 1 if bit 7 has a carry, 0 otherwise
    fn adc(&mut self, r: u8) {
        let c = u8::from(self.reg.get_flag(Flag::C));
        let v = self.reg.a.wrapping_add(r).wrapping_add(c);
        // Evaluate flags
        self.reg.set_flag(Flag::Z, v == 0);
        self.reg.set_flag(Flag::N, false);
        self.reg.set_flag(
            Flag::H,
            (self.reg.a & 0x0F) + (r & 0x0F) + (c & 0x0F) > 0x0F,
        );
        self.reg.set_flag(
            Flag::C,
            u16::from(self.reg.a) + u16::from(r) + u16::from(c) > 0xFF,
        );
        self.reg.a = v;
    }

    /// Adds an immediate value as a signed 8-bit integer to the
    /// Stack Pointer (SP).
    /// Flags:
    ///
    /// - Z: Set to 0
    /// - N: Set to 0
    /// - H: Set to 1 if bit 3 carries, 0 otherwise
    /// - C: Set to 1 if bit 7 carries, 0 otherwise
    fn add_sp(&mut self, mmu: &mut mmu::Mmu) {
        let v = (i16::from(self.imm(mmu) as i8)) as u16;
        self.reg.set_flag(Flag::Z, false);
        self.reg.set_flag(Flag::N, false);
        self.reg
            .set_flag(Flag::H, (self.reg.sp & 0x000F) + (v & 0x000F) > 0x000F);
        self.reg
            .set_flag(Flag::C, (self.reg.sp & 0x00FF) + (v & 0x00FF) > 0x00FF);
        self.reg.sp = self.reg.sp.wrapping_add(v);
    }

    /// Adds a given 16-bit register value to the HL register.
    /// Flags:
    ///
    /// - Z: Set to 0
    /// - N: Set to 0
    /// - H: Set to 1 if bit 3 carries, 0 otherwise
    /// - C: Set to 1 if bit 7 carries, 0 otherwise
    fn add_hl(&mut self, r: u16) {
        let hl = self.reg.get_hl();
        self.reg.set_flag(Flag::N, false);
        self.reg
            .set_flag(Flag::H, (r & 0x000F) + (hl & 0x000F) > 0x000F);
        self.reg
            .set_flag(Flag::C, (r & 0x00FF) + (hl & 0x00FF) > 0x00FF);
        self.reg.set_hl(hl.wrapping_add(r));
    }

    /// Subtracts the given register value `r` from the `A` register.
    /// Flags:
    ///
    /// - Z: Set to 1 if resulting value is 0, set to 0 otherwise
    /// - N: Set to 1
    /// - H: Set to 1 if bit 3 doesn't borrow, 0 otherwise
    /// - C: Set to 1 if bit 7 doesn't borrow, 0 otherwise
    fn sub(&mut self, r: u8) {
        let v = self.reg.a.wrapping_sub(r);
        // Evaluate flags
        self.reg.set_flag(Flag::Z, v == 0);
        self.reg.set_flag(Flag::N, true);
        self.reg.set_flag(Flag::H, (self.reg.a & 0x0F) < (r & 0x0F));
        self.reg
            .set_flag(Flag::C, u16::from(self.reg.a) < u16::from(r));
        self.reg.a = v;
    }

    /// Subtracts the given register value `r` plus the carry
    /// from the `A` register.
    /// Flags:
    ///
    /// - Z: Set to 1 if resulting value is 0, set to 0 otherwise
    /// - N: Set to 1
    /// - H: Set to 1 if bit 3 doesn't borrow, 0 otherwise
    /// - C: Set to 1 if bit 7 doesn't borrow, 0 otherwise
    fn sbc(&mut self, r: u8) {
        let c = u8::from(self.reg.get_flag(Flag::C));
        let v = self.reg.a.wrapping_sub(r).wrapping_sub(c);
        // Evaluate flags
        self.reg.set_flag(Flag::Z, v == 0);
        self.reg.set_flag(Flag::N, true);
        self.reg
            .set_flag(Flag::H, (self.reg.a & 0x0F) < (r & 0x0F) + (c & 0x0F));
        self.reg
            .set_flag(Flag::C, u16::from(self.reg.a) < u16::from(r) + u16::from(c));
        self.reg.a = v;
    }

    /// Performs a bitwise AND operation between `A` and the given register `r`
    /// Flags:
    ///
    /// - Z: Set to 1 if resulting value is 0, set to 0 otherwise
    /// - N: Set to 0
    /// - H: Set to 1
    /// - C: Set to 0
    fn and(&mut self, r: u8) {
        let v = self.reg.a & r;
        self.reg.set_flag(Flag::Z, v == 0);
        self.reg.set_flag(Flag::N, false);
        self.reg.set_flag(Flag::H, true);
        self.reg.set_flag(Flag::C, false);
        self.reg.a = v;
    }

    /// Performs a bitwise XOR operation between `A` and the given register `r`
    /// Flags:
    ///
    /// - Z: Set to 1 if resulting value is 0, set to 0 otherwise
    /// - N: Set to 0
    /// - H: Set to 0
    /// - C: Set to 0
    fn xor(&mut self, r: u8) {
        let v = self.reg.a ^ r;
        self.reg.set_flag(Flag::Z, v == 0);
        self.reg.set_flag(Flag::N, false);
        self.reg.set_flag(Flag::H, false);
        self.reg.set_flag(Flag::C, false);
        self.reg.a = v;
    }

    /// Performs a bitwise OR operation between `A` and the given register `r`
    /// Flags:
    ///
    /// - Z: Set to 1 if resulting value is 0, set to 0 otherwise
    /// - N: Set to 0
    /// - H: Set to 0
    /// - C: Set to 0
    fn or(&mut self, r: u8) {
        let v = self.reg.a | r;
        self.reg.set_flag(Flag::Z, v == 0);
        self.reg.set_flag(Flag::N, false);
        self.reg.set_flag(Flag::H, false);
        self.reg.set_flag(Flag::C, false);
        self.reg.a = v;
    }

    /// Performs a compare operation between `A` and the given register `r`
    /// Sets the flags similar to a SUB operation, but not writing the result
    /// Flags:
    ///
    /// - Z: Set to 1 if resulting value is 0, set to 0 otherwise
    /// - N: Set to 1
    /// - H: Set to 1 if bit 3 doesn't borrow, 0 otherwise
    /// - C: Set to 1 if bit 7 doesn't borrow, 0 otherwise
    fn cp(&mut self, r: u8) {
        // Save current value of `A` to revert after SUB
        let a = self.reg.a;
        self.sub(r);
        self.reg.a = a;
    }

    /// Increment the given value `r` and returns the incremented value.
    /// Flags:
    ///
    /// - Z: Set to 1 if resulting value is 0, set to 0 otherwise
    /// - N: Set to 0
    /// - H: Set to 1 if bit 3 carries, 0 otherwise
    /// - C: None
    fn inc(&mut self, r: u8) -> u8 {
        let v = r.wrapping_add(1);
        self.reg.set_flag(Flag::Z, v == 0);
        self.reg.set_flag(Flag::N, false);
        self.reg.set_flag(Flag::H, (r & 0x0F) + 0x1 > 0x0F);
        v
    }

    /// Decrement the given value `r` and returns the incremented value.
    /// Flags:
    ///
    /// - Z: Set to 1 if resulting value is 0, set to 0 otherwise
    /// - N: Set to 1
    /// - H: Set to 1 if bit 3 doesn't borrow, 0 otherwise
    /// - C: None
    fn dec(&mut self, r: u8) -> u8 {
        let v = r.wrapping_sub(1);
        self.reg.set_flag(Flag::Z, v == 0);
        self.reg.set_flag(Flag::N, true);
        self.reg.set_flag(Flag::H, r.trailing_zeros() >= 4);
        v
    }

    /// Rotate the given register value left, with bit 7 wrapping to bit 0
    /// Flags:
    ///
    /// - Z: Set to 1 if resulting value is 0, set to 0 otherwise
    /// - N: Set to 0
    /// - H: Set to 0
    /// - C: Set to value of `r` bit 7, before the shift
    fn rlc(&mut self, r: u8) -> u8 {
        let v = r.rotate_left(1);
        self.reg.set_flag(Flag::Z, v == 0);
        self.reg.set_flag(Flag::N, false);
        self.reg.set_flag(Flag::H, false);
        self.reg.set_flag(Flag::C, (r >> 7) == 0x1);
        v
    }

    /// Rotate the given register value right, with bit 0 wrapping to bit 7
    /// Flags:
    ///
    /// - Z: Set to 1 if resulting value is 0, set to 0 otherwise
    /// - N: Set to 0
    /// - H: Set to 0
    /// - C: Set to value of `r` bit 0, before the shift
    fn rrc(&mut self, r: u8) -> u8 {
        let v = r.rotate_right(1);
        self.reg.set_flag(Flag::Z, v == 0);
        self.reg.set_flag(Flag::N, false);
        self.reg.set_flag(Flag::H, false);
        self.reg.set_flag(Flag::C, (r & 0x01) == 0x1);
        v
    }

    /// Rotate the given register value left, with bit 7 set to C,
    /// and bit 0 containing the value of the old C.
    /// Flags:
    ///
    /// - Z: Set to 1 if resulting value is 0, set to 0 otherwise
    /// - N: Set to 0
    /// - H: Set to 0
    /// - C: Set to value of `r` bit 7, before the shift
    fn rl(&mut self, r: u8) -> u8 {
        let mut v = r.rotate_left(1);
        v = v | (self.reg.get_flag(Flag::C) as u8);
        self.reg.set_flag(Flag::Z, v == 0);
        self.reg.set_flag(Flag::N, false);
        self.reg.set_flag(Flag::H, false);
        self.reg.set_flag(Flag::C, (r >> 7) == 0x1);
        v
    }

    /// Rotate the given register value right, with bit 0 wrapping to bit 7
    /// Flags:
    ///
    /// - Z: Set to 1 if resulting value is 0, set to 0 otherwise
    /// - N: Set to 0
    /// - H: Set to 0
    /// - C: Set to value of `r` bit 0, before the shift
    fn rr(&mut self, r: u8) -> u8 {
        let v = r.rotate_right(1);
        self.reg.set_flag(Flag::Z, v == 0);
        self.reg.set_flag(Flag::N, false);
        self.reg.set_flag(Flag::H, false);
        self.reg.set_flag(Flag::C, (r & 0x01) == 0x1);
        v
    }
}

#[cfg(test)]
mod cpu_tests {
    use super::*;
    #[test]
    fn register_read() {
        let reg = Registers::power_on();

        // Verify power-on values
        assert_eq!(reg.a, 0x01);
        assert_eq!(reg.f, 0xB0);
        assert_eq!(reg.b, 0x00);
        assert_eq!(reg.c, 0x13);
        assert_eq!(reg.d, 0x00);
        assert_eq!(reg.e, 0xD8);
        assert_eq!(reg.h, 0x01);
        assert_eq!(reg.l, 0x4D);
        assert_eq!(reg.sp, 0xFFFE);
        assert_eq!(reg.pc, 0x0100);

        // Use register pair accessors
        assert_eq!(reg.get_af(), 0x01B0);
        assert_eq!(reg.get_bc(), 0x0013);
        assert_eq!(reg.get_de(), 0x00D8);
        assert_eq!(reg.get_hl(), 0x014D);
    }

    #[test]
    fn register_write() {
        let mut reg = Registers::power_on();

        // Set register pair values
        reg.set_af(0x1234);
        reg.set_bc(0x5678);
        reg.set_de(0x9001);
        reg.set_hl(0x2345);
        assert_eq!(reg.a, 0x12);
        assert_eq!(reg.f, 0x34);
        assert_eq!(reg.b, 0x56);
        assert_eq!(reg.c, 0x78);
        assert_eq!(reg.d, 0x90);
        assert_eq!(reg.e, 0x01);
        assert_eq!(reg.h, 0x23);
        assert_eq!(reg.l, 0x45);
    }

    #[test]
    fn rl_test() {
        let mut cpu = Cpu::power_on();
        let mut v = cpu.rl(0b0110_0101);
        assert_eq!(v, 0b1100_1011);
        assert_eq!(cpu.reg.get_flag(Flag::C), false);
        v = cpu.rl(0b1100_1011);
        assert_eq!(v, 0b1001_0110);
        assert_eq!(cpu.reg.get_flag(Flag::C), true);
        v = cpu.rl(0b1001_0110);
        assert_eq!(v, 0b0010_1101);
        assert_eq!(cpu.reg.get_flag(Flag::C), true);
    }
}
