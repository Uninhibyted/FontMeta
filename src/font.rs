use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use ttf_parser::Face;

use crate::{
    app::{Field, FontFile, FontInfo, FIELDS},
    binary::*,
};

// OS/2 table offsets (defines weight, width, and style flags)
const OS2_WEIGHT_OFFSET: usize = 4;
const OS2_WIDTH_OFFSET: usize = 6;
const OS2_FS_SELECTION_OFFSET: usize = 62;

// head table offset (defines macOS-specific style flags)
const HEAD_MAC_STYLE_OFFSET: usize = 44;

// OS/2 fsSelection bit flags
const FS_ITALIC: u16 = 1 << 0;
const FS_BOLD: u16 = 1 << 5;
const FS_REGULAR: u16 = 1 << 6;
const FS_OBLIQUE: u16 = 1 << 9;

// head table macStyle bit flags (legacy macOS compatibility)
const MAC_BOLD: u16 = 1 << 0;
const MAC_ITALIC: u16 = 1 << 1;

#[derive(Clone)]
struct TableRecord {
    tag: [u8; 4],
    checksum: u32,
    offset: u32,
    length: u32,
}

#[derive(Clone)]
struct NameRecord {
    platform_id: u16,
    encoding_id: u16,
    language_id: u16,
    name_id: u16,
    length: u16,
    offset: u16,
}

pub fn is_variable_font(data: &[u8]) -> Result<bool> {
    let tables = read_table_records(data)?;
    Ok(tables.iter().any(|t| &t.tag == b"fvar"))
}

pub fn load_font(path: PathBuf) -> Result<FontFile> {
    let data = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let face = Face::parse(&data, 0)?;

    let mut info = FontInfo::default();

    for field in FIELDS {
        if let Some(name_id) = field.name_id() {
            let value = get_name(&face, name_id);
            info.set(field, value);
        }
    }

    read_style_info(&data, &mut info)?;
    let variable = is_variable_font(&data).unwrap_or(false);

    Ok(FontFile {
        path,
        original: info.clone(),
        edited: info,
        variable,
    })
}

pub fn save_fixed_font(font: &FontFile, output_dir: &PathBuf) -> Result<PathBuf> {
    let data = fs::read(&font.path)?;
    let output = rewrite_font_tables(&data, &font.edited)?;

    let file_name = font
        .path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("fixed-font.ttf");

    let out_path = output_dir.join(file_name);

    fs::write(&out_path, output)?;

    Ok(out_path)
}

fn read_style_info(font_data: &[u8], info: &mut FontInfo) -> Result<()> {
    let tables = read_table_records(font_data)?;

    if let Some(os2) = tables.iter().find(|t| &t.tag == b"OS/2") {
        let start = os2.offset as usize;
        let end = start + os2.length as usize;

        if end <= font_data.len() && os2.length as usize >= OS2_FS_SELECTION_OFFSET + 2 {
            info.set(Field::WeightClass, read_u16(font_data, start + OS2_WEIGHT_OFFSET)?.to_string());
            info.set(Field::WidthClass, read_u16(font_data, start + OS2_WIDTH_OFFSET)?.to_string());

            let fs = read_u16(font_data, start + OS2_FS_SELECTION_OFFSET)?;

            info.set(Field::ItalicFlag, (fs & FS_ITALIC != 0).to_string());
            info.set(Field::BoldFlag, (fs & FS_BOLD != 0).to_string());
            info.set(Field::RegularFlag, (fs & FS_REGULAR != 0).to_string());
            info.set(Field::ObliqueFlag, (fs & FS_OBLIQUE != 0).to_string());
        }
    }

    if let Some(head) = tables.iter().find(|t| &t.tag == b"head") {
        let start = head.offset as usize;
        let end = start + head.length as usize;

        if end <= font_data.len() && head.length as usize >= HEAD_MAC_STYLE_OFFSET + 2 {
            let mac_style = read_u16(font_data, start + HEAD_MAC_STYLE_OFFSET)?;

            if mac_style & MAC_BOLD != 0 {
                info.set(Field::BoldFlag, "true".to_string());
            }

            if mac_style & MAC_ITALIC != 0 {
                info.set(Field::ItalicFlag, "true".to_string());
            }
        }
    }

    Ok(())
}

fn get_name(face: &Face, id: u16) -> String {
    let mut fallback = None;
    for name in face.names() {
        if name.name_id != id {
            continue;
        }
        if let Some(value) = name.to_string() {
            if !value.trim().is_empty() {
                if name.is_unicode() {
                    return value;
                }
                if fallback.is_none() {
                    fallback = Some(value);
                }
            }
        }
    }
    fallback.unwrap_or_default()
}

fn rewrite_font_tables(font_data: &[u8], info: &FontInfo) -> Result<Vec<u8>> {
    if font_data.len() < 12 {
        anyhow::bail!("Invalid font");
    }

    let scaler = read_u32(font_data, 0)?;
    let tables = read_table_records(font_data)?;
    let num_tables = tables.len();
    let table_dir_len = num_tables * 16;

    let name_index = tables
        .iter()
        .position(|t| &t.tag == b"name")
        .ok_or_else(|| anyhow::anyhow!("No name table found"))?;

    let name_record = &tables[name_index];
    let name_start = name_record.offset as usize;
    let name_end = name_start + name_record.length as usize;

    if name_end > font_data.len() {
        anyhow::bail!("Invalid name table bounds");
    }

    let old_name_table = &font_data[name_start..name_end];
    let new_name_table = build_name_table(old_name_table, info)?;

    let mut table_datas: Vec<Vec<u8>> = Vec::new();

    for (i, table) in tables.iter().enumerate() {
        let start = table.offset as usize;
        let end = start + table.length as usize;

        if end > font_data.len() {
            anyhow::bail!("Invalid table bounds");
        }

        let original = font_data[start..end].to_vec();

        let data = if i == name_index {
            new_name_table.clone()
        } else if &table.tag == b"OS/2" {
            patch_os2_table(original, info)
        } else if &table.tag == b"head" {
            patch_head_table(original, info)
        } else {
            original
        };

        table_datas.push(data);
    }

    let search_range_power = largest_power_of_two(num_tables as u16);
    let search_range = search_range_power * 16;
    let entry_selector = search_range_power.trailing_zeros() as u16;
    let range_shift = (num_tables as u16 * 16) - search_range;

    let mut output = Vec::new();

    write_u32(&mut output, scaler);
    write_u16(&mut output, num_tables as u16);
    write_u16(&mut output, search_range);
    write_u16(&mut output, entry_selector);
    write_u16(&mut output, range_shift);

    let directory_pos = output.len();
    output.resize(directory_pos + table_dir_len, 0);

    let mut new_records = tables.clone();

    for (i, mut table_data) in table_datas.into_iter().enumerate() {
        while output.len() % 4 != 0 {
            output.push(0);
        }

        let offset = output.len() as u32;

        if &new_records[i].tag == b"head" && table_data.len() >= 12 {
            table_data[8..12].fill(0);
        }

        let length = table_data.len() as u32;
        let checksum = calc_checksum(&table_data);

        output.extend_from_slice(&table_data);

        while output.len() % 4 != 0 {
            output.push(0);
        }

        new_records[i].offset = offset;
        new_records[i].length = length;
        new_records[i].checksum = checksum;
    }

    for (i, table) in new_records.iter().enumerate() {
        let pos = directory_pos + i * 16;

        output[pos..pos + 4].copy_from_slice(&table.tag);
        write_u32_at(&mut output, pos + 4, table.checksum);
        write_u32_at(&mut output, pos + 8, table.offset);
        write_u32_at(&mut output, pos + 12, table.length);
    }

    fix_checksum_adjustment(&mut output, &new_records)?;

    Ok(output)
}

fn read_table_records(font_data: &[u8]) -> Result<Vec<TableRecord>> {
    if font_data.len() < 12 {
        anyhow::bail!("Invalid font");
    }

    let num_tables = read_u16(font_data, 4)? as usize;
    let table_dir_start = 12;
    let table_dir_len = num_tables * 16;

    if font_data.len() < table_dir_start + table_dir_len {
        anyhow::bail!("Invalid table directory");
    }

    let mut tables = Vec::new();

    for i in 0..num_tables {
        let pos = table_dir_start + i * 16;

        tables.push(TableRecord {
            tag: font_data[pos..pos + 4].try_into().map_err(|_| anyhow::anyhow!("Invalid tag slice"))?,
            checksum: read_u32(font_data, pos + 4)?,
            offset: read_u32(font_data, pos + 8)?,
            length: read_u32(font_data, pos + 12)?,
        });
    }

    Ok(tables)
}

fn build_name_table(old: &[u8], info: &FontInfo) -> Result<Vec<u8>> {
    if old.len() < 6 {
        anyhow::bail!("Invalid name table");
    }

    let format = read_u16(old, 0)?;
    let count = read_u16(old, 2)? as usize;
    let old_string_offset = read_u16(old, 4)? as usize;

    if format > 1 {
        anyhow::bail!("Unsupported name table format {format}");
    }

    let records_start = 6;
    let records_len = count * 12;

    if old.len() < records_start + records_len {
        anyhow::bail!("Invalid name records");
    }

    let mut records = Vec::new();

    for i in 0..count {
        let pos = records_start + i * 12;

        records.push(NameRecord {
            platform_id: read_u16(old, pos)?,
            encoding_id: read_u16(old, pos + 2)?,
            language_id: read_u16(old, pos + 4)?,
            name_id: read_u16(old, pos + 6)?,
            length: read_u16(old, pos + 8)?,
            offset: read_u16(old, pos + 10)?,
        });
    }

    let mut lang_tag_records = Vec::new();

    let lang_tag_count_pos = records_start + records_len;
    let lang_tag_count = if format == 1 {
        if old.len() < lang_tag_count_pos + 2 {
            anyhow::bail!("Invalid format 1 name table");
        }

        read_u16(old, lang_tag_count_pos)? as usize
    } else {
        0
    };

    if format == 1 {
        let lang_records_start = lang_tag_count_pos + 2;

        if old.len() < lang_records_start + lang_tag_count * 4 {
            anyhow::bail!("Invalid language tag records");
        }

        for i in 0..lang_tag_count {
            let pos = lang_records_start + i * 4;
            lang_tag_records.push((read_u16(old, pos)?, read_u16(old, pos + 2)?));
        }
    }

    let new_string_offset = if format == 0 {
        6 + records_len
    } else {
        6 + records_len + 2 + lang_tag_count * 4
    };

    let mut strings = Vec::new();
    let mut new_records = Vec::new();

    for mut record in records {
        let value = replacement_for_name_id(record.name_id, info);

        let bytes = if let Some(value) = value {
            encode_name_value(&value, record.platform_id)
        } else {
            extract_old_string(old, old_string_offset, record.offset, record.length)?
        };

        record.offset = strings.len() as u16;
        record.length = bytes.len() as u16;

        strings.extend_from_slice(&bytes);
        new_records.push(record);
    }

    let mut new_lang_tags = Vec::new();

    for (length, offset) in lang_tag_records {
        let bytes = extract_old_string(old, old_string_offset, offset, length)?;
        let new_offset = strings.len() as u16;
        let new_length = bytes.len() as u16;

        strings.extend_from_slice(&bytes);
        new_lang_tags.push((new_length, new_offset));
    }

    let mut out = Vec::new();

    write_u16(&mut out, format);
    write_u16(&mut out, count as u16);
    write_u16(&mut out, new_string_offset as u16);

    for record in new_records {
        write_u16(&mut out, record.platform_id);
        write_u16(&mut out, record.encoding_id);
        write_u16(&mut out, record.language_id);
        write_u16(&mut out, record.name_id);
        write_u16(&mut out, record.length);
        write_u16(&mut out, record.offset);
    }

    if format == 1 {
        write_u16(&mut out, new_lang_tags.len() as u16);

        for (length, offset) in new_lang_tags {
            write_u16(&mut out, length);
            write_u16(&mut out, offset);
        }
    }

    out.extend_from_slice(&strings);

    Ok(out)
}

fn replacement_for_name_id(id: u16, info: &FontInfo) -> Option<String> {
    FIELDS.iter().find(|f| f.name_id() == Some(id)).map(|f| info.get(*f))
}

// Platform 0/3 (Unicode/Windows) use UTF-16BE; others use raw bytes.
fn encode_name_value(value: &str, platform_id: u16) -> Vec<u8> {
    if platform_id == 0 || platform_id == 3 {
        value.encode_utf16().flat_map(|unit| unit.to_be_bytes()).collect()
    } else {
        value.bytes().collect()
    }
}

fn extract_old_string(old: &[u8], string_offset: usize, offset: u16, length: u16) -> Result<Vec<u8>> {
    let start = string_offset + offset as usize;
    let end = start + length as usize;

    if end > old.len() {
        anyhow::bail!("Invalid name string bounds");
    }

    Ok(old[start..end].to_vec())
}

fn patch_os2_table(mut data: Vec<u8>, info: &FontInfo) -> Vec<u8> {
    if data.len() < OS2_FS_SELECTION_OFFSET + 2 {
        return data;
    }

    write_u16_at(&mut data, OS2_WEIGHT_OFFSET, info.get_u16(Field::WeightClass));
    write_u16_at(&mut data, OS2_WIDTH_OFFSET, info.get_u16(Field::WidthClass));

    let mut fs = read_u16(&data, OS2_FS_SELECTION_OFFSET).unwrap_or(0);

    set_bit(&mut fs, FS_BOLD, info.get_bool(Field::BoldFlag));
    set_bit(&mut fs, FS_ITALIC, info.get_bool(Field::ItalicFlag));
    set_bit(&mut fs, FS_OBLIQUE, info.get_bool(Field::ObliqueFlag));
    set_bit(&mut fs, FS_REGULAR, info.get_bool(Field::RegularFlag));

    write_u16_at(&mut data, OS2_FS_SELECTION_OFFSET, fs);

    data
}

fn patch_head_table(mut data: Vec<u8>, info: &FontInfo) -> Vec<u8> {
    if data.len() < HEAD_MAC_STYLE_OFFSET + 2 {
        return data;
    }

    let mut mac_style = read_u16(&data, HEAD_MAC_STYLE_OFFSET).unwrap_or(0);

    set_bit(&mut mac_style, MAC_BOLD, info.get_bool(Field::BoldFlag));
    set_bit(&mut mac_style, MAC_ITALIC, info.get_bool(Field::ItalicFlag) || info.get_bool(Field::ObliqueFlag));

    write_u16_at(&mut data, HEAD_MAC_STYLE_OFFSET, mac_style);

    data
}

fn set_bit(value: &mut u16, bit: u16, enabled: bool) {
    if enabled {
        *value |= bit;
    } else {
        *value &= !bit;
    }
}

fn fix_checksum_adjustment(font: &mut Vec<u8>, tables: &[TableRecord]) -> Result<()> {
    let head = tables
        .iter()
        .find(|t| &t.tag == b"head")
        .ok_or_else(|| anyhow::anyhow!("No head table found"))?;

    let checksum_adjustment_pos = head.offset as usize + 8;

    if checksum_adjustment_pos + 4 > font.len() {
        anyhow::bail!("Invalid head table");
    }

    write_u32_at(font, checksum_adjustment_pos, 0);

    let checksum = calc_checksum(font);
    let adjustment = 0xB1B0AFBAu32.wrapping_sub(checksum);

    write_u32_at(font, checksum_adjustment_pos, adjustment);

    Ok(())
}
