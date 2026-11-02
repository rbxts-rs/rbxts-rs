//! A reader of the bytecode Luau's compiler writes (`BytecodeBuilder::finalize`), as the pinned release's loader
//! reads it (`VM/src/lvmload.cpp`, `luau_load`): bytecode versions 3 to 14 and the experimental classes version
//! (100), type encoding versions 1 to 3. Only what [`crate::function_stats`] and [`crate::debug_locals`] report is
//! kept; everything else is skipped, field by field, so a malformed or unknown layout is an error, never a guess.

/// One function prototype, in bytecode order.
pub(crate) struct Proto {
    pub max_stack_size: u8,
    pub num_params: u8,
    pub num_upvalues: u8,
    pub line_defined: u32,
    /// The debug locals (debug level 2): name, start and end pc, register. Empty below debug level 2.
    pub locals: Vec<(String, u32, u32, u8)>,
}

/// `LBC_CONSTANT_*` of `Common/include/Luau/Bytecode.h`.
mod constant {
    pub const NIL: u8 = 0;
    pub const BOOLEAN: u8 = 1;
    pub const NUMBER: u8 = 2;
    pub const STRING: u8 = 3;
    pub const IMPORT: u8 = 4;
    pub const TABLE: u8 = 5;
    pub const CLOSURE: u8 = 6;
    pub const VECTOR: u8 = 7;
    pub const TABLE_WITH_CONSTANTS: u8 = 8;
    pub const INTEGER: u8 = 9;
    pub const CLASS_SHAPE: u8 = 10;
    pub const VECTORD: u8 = 11;
}

/// `LPF_INLINABLE`: the prototype records its inlining cost (version 12 and later).
const LPF_INLINABLE: u8 = 1 << 3;

struct Reader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn byte(&mut self) -> Result<u8, String> {
        let byte = *self.data.get(self.offset).ok_or("bytecode ends early")?;
        self.offset += 1;
        Ok(byte)
    }

    fn skip(&mut self, count: usize) -> Result<(), String> {
        let end = self.offset.checked_add(count).filter(|&end| end <= self.data.len()).ok_or("bytecode ends early")?;
        self.offset = end;
        Ok(())
    }

    fn bytes(&mut self, count: usize) -> Result<&'a [u8], String> {
        let start = self.offset;
        self.skip(count)?;
        Ok(&self.data[start..self.offset])
    }

    fn var_int(&mut self) -> Result<u32, String> {
        let value = self.var_int64()?;
        u32::try_from(value).map_err(|_| "a varint past 32 bits".to_owned())
    }

    fn var_int64(&mut self) -> Result<u64, String> {
        let mut result = 0u64;
        let mut shift = 0;
        loop {
            let byte = self.byte()?;
            if shift < 64 {
                result |= u64::from(byte & 127) << shift;
            }
            shift += 7;
            if byte & 128 == 0 {
                return Ok(result);
            }
        }
    }

    fn count(&mut self) -> Result<usize, String> {
        Ok(self.var_int()? as usize)
    }
}

/// Reads every prototype of `bytecode` (a successful compile's output: its first byte is the version, not 0).
pub(crate) fn read(bytecode: &[u8]) -> Result<Vec<Proto>, String> {
    let mut r = Reader { data: bytecode, offset: 0 };
    let version = r.byte()?;
    if !(3..=14).contains(&version) && version != 100 {
        return Err(format!("bytecode version {version} is not one this reader knows"));
    }
    let types_version = if version >= 4 { r.byte()? } else { 0 };
    if version >= 4 && !(1..=3).contains(&types_version) {
        return Err(format!("type encoding version {types_version} is not one this reader knows"));
    }
    let string_count = r.count()?;
    let mut strings = Vec::with_capacity(string_count);
    for _ in 0..string_count {
        let length = r.count()?;
        strings.push(String::from_utf8_lossy(r.bytes(length)?).into_owned());
    }
    // the userdata type remapping: index (0 ends it), then a string reference
    if types_version == 3 {
        while r.byte()? != 0 {
            r.var_int()?;
        }
    }
    let string = |id: u32| -> Result<String, String> {
        match id {
            0 => Ok(String::new()),
            id => strings.get(id as usize - 1).cloned().ok_or_else(|| format!("string reference {id} out of range")),
        }
    };

    let proto_count = r.count()?;
    let mut protos = Vec::with_capacity(proto_count);
    for _ in 0..proto_count {
        // version 12 and later write each prototype's size first (a loader skips what it does not know)
        let sized = if version >= 12 { Some(r.count()?) } else { None };
        let start = r.offset;
        let max_stack_size = r.byte()?;
        let num_params = r.byte()?;
        let num_upvalues = r.byte()?;
        let _is_vararg = r.byte()?;
        let mut flags = 0;
        if version >= 4 {
            flags = r.byte()?;
            let type_size = r.count()?;
            r.skip(type_size)?;
        }
        let code_size = r.count()?;
        r.skip(code_size.checked_mul(4).ok_or("code size overflows")?)?;
        let constant_count = r.count()?;
        for _ in 0..constant_count {
            match r.byte()? {
                constant::NIL => {}
                constant::BOOLEAN => r.skip(1)?,
                constant::NUMBER => r.skip(8)?,
                constant::STRING | constant::CLOSURE => {
                    r.var_int()?;
                }
                constant::IMPORT => r.skip(4)?,
                constant::TABLE => {
                    for _ in 0..r.count()? {
                        r.var_int()?;
                    }
                }
                constant::VECTOR => r.skip(16)?,
                constant::TABLE_WITH_CONSTANTS => {
                    for _ in 0..r.count()? {
                        r.var_int()?;
                        r.skip(4)?;
                    }
                }
                constant::INTEGER => {
                    r.skip(1)?;
                    r.var_int64()?;
                }
                constant::CLASS_SHAPE => {
                    r.var_int()?;
                    let members = r.count()? + r.count()?;
                    for _ in 0..members {
                        r.var_int()?;
                    }
                }
                constant::VECTORD => r.skip(32)?,
                kind => return Err(format!("constant kind {kind} is not one this reader knows")),
            }
        }
        for _ in 0..r.count()? {
            r.var_int()?;
        }
        let line_defined = r.var_int()?;
        let _debug_name = r.var_int()?;
        if r.byte()? != 0 {
            let line_gap_log2 = r.byte()?;
            if line_gap_log2 >= 32 {
                return Err(format!("line gap 2^{line_gap_log2}"));
            }
            let intervals = if code_size == 0 { 0 } else { ((code_size - 1) >> line_gap_log2) + 1 };
            r.skip(code_size)?;
            r.skip(intervals * 4)?;
        }
        let mut locals = Vec::new();
        if r.byte()? != 0 {
            for _ in 0..r.count()? {
                let name = string(r.var_int()?)?;
                let start_pc = r.var_int()?;
                let end_pc = r.var_int()?;
                let register = r.byte()?;
                locals.push((name, start_pc, end_pc, register));
            }
            for _ in 0..r.count()? {
                r.var_int()?;
            }
        }
        if version >= 11 {
            for _ in 0..r.count()? {
                r.skip(1)?;
                r.var_int()?;
            }
        }
        if version >= 12 && flags & LPF_INLINABLE != 0 {
            r.var_int64()?;
        }
        if let Some(size) = sized {
            let end = start.checked_add(size).filter(|&end| end <= bytecode.len() && end >= r.offset);
            r.offset = end.ok_or("a prototype's size disagrees with its contents")?;
        }
        protos.push(Proto { max_stack_size, num_params, num_upvalues, line_defined, locals });
    }
    let main = r.var_int()?;
    if main as usize >= protos.len() {
        return Err(format!("main prototype {main} out of range"));
    }
    Ok(protos)
}
