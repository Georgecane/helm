//! Native structure-discovery candidate transforms for Helm.
//! Every transform is reversible; no source information is discarded.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Transform { Identity=0, Delta1=1, Delta2=2, Delta4=3, Delta8=4, GaiaContext=5, MultivariableMean=6 }
impl Transform {
    fn all() -> [Self; 7] { [Self::Identity,Self::Delta1,Self::Delta2,Self::Delta4,Self::Delta8,Self::GaiaContext,Self::MultivariableMean] }
    fn from_id(id:u8)->Option<Self>{Self::all().into_iter().find(|t|*t as u8==id)}
}
fn stride(t:Transform)->Option<usize>{match t{Transform::Delta1=>Some(1),Transform::Delta2=>Some(2),Transform::Delta4=>Some(4),Transform::Delta8=>Some(8),_=>None}}

// Round 1: SMDR-style symbolic residuals at candidate byte-column strides.
// Round 2: Gaia's adaptive successor model, updated identically by the decoder.
// Round 3: Multivariable Mean's two-symbol local predictor.
// Round 4: candidate costs are compared by the token coder and exact round trips.
pub(super) fn candidates(input:&[u8])->Vec<(u8,Vec<u8>)>{
    let mut out=vec![(Transform::Identity as u8,input.to_vec())];
    for t in [Transform::Delta1,Transform::Delta2,Transform::Delta4,Transform::Delta8,Transform::GaiaContext,Transform::MultivariableMean]{out.push((t as u8,forward(input,t)));}
    out
}
fn forward(input:&[u8],t:Transform)->Vec<u8>{
    if let Some(step)=stride(t){let mut out=input.to_vec();for i in step..input.len(){out[i]=input[i].wrapping_sub(input[i-step]);}return out;}
    match t {
        Transform::Identity=>input.to_vec(),
        Transform::GaiaContext=>{let mut out=Vec::with_capacity(input.len());let mut counts=[[0u32;256];256];let mut best=[0u8;256];let mut best_count=[0u32;256];let mut prev=0u8;for &symbol in input{out.push(symbol.wrapping_sub(best[prev as usize]));let r=prev as usize;let c=symbol as usize;counts[r][c]=counts[r][c].saturating_add(1);if counts[r][c]>best_count[r]{best_count[r]=counts[r][c];best[r]=symbol;}prev=symbol;}out},
        Transform::MultivariableMean=>input.iter().enumerate().map(|(i,&b)|{let p=if i>=2{((u16::from(input[i-1])+u16::from(input[i-2]))/2) as u8}else if i==1{input[0]}else{0};b.wrapping_sub(p)}).collect(),
        _=>unreachable!(),
    }
}
pub(super) fn inverse(input:&[u8],id:u8)->Result<Vec<u8>,&'static str>{
    let t=Transform::from_id(id).ok_or("unknown structure transform id")?;
    if t==Transform::Identity{return Ok(input.to_vec());}
    if let Some(step)=stride(t){let mut out=input.to_vec();for i in step..out.len(){out[i]=out[i].wrapping_add(out[i-step]);}return Ok(out);}
    match t {
        Transform::GaiaContext=>{let mut out=Vec::with_capacity(input.len());let mut counts=[[0u32;256];256];let mut best=[0u8;256];let mut best_count=[0u32;256];let mut prev=0u8;for &residual in input{let symbol=residual.wrapping_add(best[prev as usize]);out.push(symbol);let r=prev as usize;let c=symbol as usize;counts[r][c]=counts[r][c].saturating_add(1);if counts[r][c]>best_count[r]{best_count[r]=counts[r][c];best[r]=symbol;}prev=symbol;}Ok(out)},
        Transform::MultivariableMean=>{let mut out=Vec::with_capacity(input.len());for &residual in input{let i=out.len();let p=if i>=2{((u16::from(out[i-1])+u16::from(out[i-2]))/2) as u8}else if i==1{out[0]}else{0};out.push(residual.wrapping_add(p));}Ok(out)},
        _=>unreachable!(),
    }
}
#[cfg(test)] mod tests { use super::*; #[test] fn every_transform_is_reversible(){let x:Vec<u8>=(0..20000).map(|i|((i*i+i/7)%251) as u8).collect();for(id,y)in candidates(&x){assert_eq!(inverse(&y,id).unwrap(),x,"transform {id}");}} }
