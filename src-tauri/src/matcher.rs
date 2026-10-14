use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use serde::{Deserialize, Serialize};

use crate::{FileResult, Hit, Rule, Segment};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub skip_common_dirs: bool,
    pub exclude_dirs: Vec<String>,
}
impl Default for ScanOptions {
    fn default() -> Self {
        Self { case_sensitive: true, whole_word: false, skip_common_dirs: true, exclude_dirs: Vec::new() }
    }
}

pub struct Matcher {
    rules: Vec<Rule>,
    matcher: Option<AhoCorasick>,
    options: ScanOptions,
}
impl Matcher {
    pub fn new(rules: Vec<Rule>, options: ScanOptions) -> Result<Self, String> {
        let rules: Vec<_> = rules.into_iter().filter(|r|r.enabled && !r.word.trim().is_empty()).collect();
        let matcher=if rules.is_empty(){None}else{
            Some(AhoCorasickBuilder::new()
                .match_kind(MatchKind::Standard)
                .ascii_case_insensitive(!options.case_sensitive)
                .build(rules.iter().map(|r|r.word.as_str()))
                .map_err(|e|format!("无法创建关键词匹配器：{e}"))?)
        };
        Ok(Self{rules,matcher,options})
    }

    pub fn check(&self, name: String, segments: &[Segment]) -> FileResult {
        let mut seen=vec![false;self.rules.len()];
        let mut hits=Vec::new();
        let mut forbidden=false;
        if let Some(ac)=&self.matcher {
            for segment in segments {
                // Full text segments can contain newlines. The line number is found by binary search.
                for found in ac.find_overlapping_iter(&segment.text) {
                    let idx=found.pattern().as_usize();
                    if self.options.whole_word && !word_boundary(&segment.text,found.start(),found.end()) {continue;}
                    let rule=&self.rules[idx];
                    seen[idx]=true;
                    forbidden |= rule.kind=="forbidden";
                    if hits.len()>=3000 {continue;}
                    let start=left_boundary(&segment.text,found.start().saturating_sub(64));
                    let end=right_boundary(&segment.text,(found.end()+64).min(segment.text.len()));
                    let matched=&segment.text[found.start()..found.end()];
                    hits.push(Hit{
                        word:rule.word.clone(),kind:rule.kind.clone(),
                        location:segment.location_at(found.start()),
                        // Preserve original casing in excerpt even when ASCII case-insensitive.
                        context:format!("{}{}{}", &segment.text[start..found.start()], matched, &segment.text[found.end()..end]),
                        matched:matched.into(),
                    });
                }
            }
        }
        let missing=self.rules.iter().enumerate()
            .filter(|(idx,r)|r.kind=="required" && !seen[*idx])
            .map(|(_,r)|r.word.clone()).collect::<Vec<_>>();
        FileResult{path:name,status:if forbidden||!missing.is_empty(){"issue"}else{"passed"}.into(),
            hits,missing,error:None}
    }
}
fn left_boundary(s:&str,mut idx:usize)->usize {
    while !s.is_char_boundary(idx){idx-=1;}
    idx
}
fn right_boundary(s:&str,mut idx:usize)->usize {
    while !s.is_char_boundary(idx){idx+=1;}
    idx
}
fn is_word(c:char)->bool{c.is_alphanumeric()||c=='_'}
fn word_boundary(text:&str,start:usize,end:usize)->bool{
    !text[..start].chars().next_back().map(is_word).unwrap_or(false)
       && !text[end..].chars().next().map(is_word).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rule(word:&str,kind:&str)->Rule{Rule{word:word.into(),kind:kind.into(),enabled:true}}
    fn segment(text:&str)->Segment{Segment::new_lines(text.into())}
    #[test]
    fn overlapping_patterns_and_mixed_rules() {
        let matcher=Matcher::new(vec![rule("轨道","search"),rule("轨道机动","forbidden"),rule("机动","required")],ScanOptions::default()).unwrap();
        let out=matcher.check("x.rs".into(),&[segment("发现轨道机动")]);
        assert_eq!(out.hits.len(),3);
        assert_eq!(out.status,"issue");
        assert!(out.missing.is_empty());
    }
    #[test]
    fn required_missing_and_hit_limit_do_not_hide_violation(){
        let matcher=Matcher::new(vec![rule("a","search"),rule("z","forbidden"),rule("finished","required")],ScanOptions::default()).unwrap();
        let out=matcher.check("long.txt".into(),&[segment(&( "a".repeat(3100)+"z" ))]);
        assert_eq!(out.hits.len(),3000);
        assert_eq!(out.status,"issue");
        assert_eq!(out.missing,vec!["finished"]);
    }
    #[test]
    fn ascii_case_insensitive_and_word_boundaries(){
        let opts=ScanOptions{case_sensitive:false,whole_word:true,..Default::default()};
        let m=Matcher::new(vec![rule("todo","forbidden")],opts).unwrap();
        let out=m.check("x.py".into(),&[segment("atodolist TODO todo_list ToDo.")]);
        assert_eq!(out.hits.len(),2);
        assert_eq!(out.status,"issue");
    }
    #[test]
    fn perf_smoke_many_patterns(){
        let words=(0..250).map(|i|rule(&format!("keyword{i}"),"search")).collect();
        let m=Matcher::new(words,ScanOptions::default()).unwrap();
        let corpus=(0..12000).map(|i|format!("line {i}: keyword{}\n",i%250)).collect::<String>();
        let now=std::time::Instant::now();
        let out=m.check("benchmark.rs".into(),&[segment(&corpus)]);
        assert_eq!(out.hits.len(),3000);
        eprintln!("Aho-Corasick: 250 patterns, 12000 lines, {:?} (environment-dependent)",now.elapsed());
    }
    #[test]
    fn exact_case_and_lines(){
        let m=Matcher::new(vec![rule("TODO\nfix","search")],ScanOptions::default()).unwrap();
        let out=m.check("x.py".into(),&[segment("head\nTODO\nfix\nrest")]);
        assert_eq!(out.hits.len(),1);
        assert_eq!(out.hits[0].location,"第 2 行");
    }
}
